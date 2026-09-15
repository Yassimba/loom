import { dirname } from "node:path";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { checkAction } from "../../src/core";
import {
  type AllowedPath,
  canonicalizeFromCwd,
  normalizeForDisplay,
  type PathAccessState,
} from "../../src/core/paths";
import { configLoader, type ResolvedConfig } from "../../src/shared/config";
import {
  createPromptClosedPayload,
  createPromptOpenedPayload,
  GUARDRAILS_PROMPT_CLOSED_EVENT,
  GUARDRAILS_PROMPT_OPENED_EVENT,
} from "../../src/shared/events";
import { getSessionMode } from "../../src/shared/session-mode";
import type { WorkspaceRootControl } from "../guardrails/commands/add-dir";
import {
  BLOCKED_TOOLS,
  compilePolicies,
  createPolicyRules,
  protectionRank,
} from "../guardrails/rules";
import { piDocumentationPaths, temporaryPaths } from "./dynamic-resources";
import {
  createPendingGrant,
  isGrantTooBroad,
  type PendingPathGrant,
  pendingAllowedPaths,
  persistGrant,
  resolveAllowedPaths,
} from "./grants";
import { createPathAccessPromptComponent, type PromptResult } from "./prompt";
import { createPathAccessRule } from "./rules";
import { type FileTarget, targetsForTool } from "./targets";

type PolicyCheck = {
  config: ResolvedConfig;
  toolName: string;
  cwd: string;
  targets: FileTarget[];
};

async function checkPolicyTargets({
  config,
  toolName,
  cwd,
  targets,
}: PolicyCheck): Promise<{ block: true; reason: string } | undefined> {
  if (!config.features.policies) return;
  const policies = compilePolicies(config.policies.rules)
    .filter((policy) => BLOCKED_TOOLS[policy.protection].has(toolName))
    .sort((a, b) => protectionRank(b.protection) - protectionRank(a.protection));
  const rules = createPolicyRules(policies, cwd);
  for (const target of targets) {
    const safety = await checkAction(
      {
        kind: "file",
        path: target.path,
        unresolved: target.unresolved,
        origin: toolName,
      },
      rules,
    );
    if (safety.kind !== "safe") return { block: true, reason: safety.reason };
  }
}

export function registerPathAccess(pi: ExtensionAPI, workspace: WorkspaceRootControl): void {
  // Pi docs paths depend only on `PI_PACKAGE_DIR` / the package root and are
  // fixed for the process lifetime, so resolve once at setup.
  const builtInAllowedPaths = [...piDocumentationPaths(), ...temporaryPaths()];

  let currentSkillAllowedPaths: AllowedPath[] = [];

  pi.on("before_agent_start", (event) => {
    const skills = event.systemPromptOptions.skills;

    if (!skills || skills.length === 0) return;

    currentSkillAllowedPaths = skills.flatMap((skill) => [
      { kind: "file", path: skill.filePath },
      { kind: "directory", path: skill.baseDir },
    ]);
  });

  pi.on("tool_call", async (event, ctx) => {
    const sessionMode = getSessionMode();
    if (sessionMode === "yolo") return;

    const config = configLoader.getConfig();
    const input = event.input as Record<string, unknown>;
    const targets = await targetsForTool(event.toolName, input, ctx.cwd);

    const policyBlock = await checkPolicyTargets({
      config,
      toolName: event.toolName,
      cwd: ctx.cwd,
      targets,
    });
    if (policyBlock) return policyBlock;

    if (sessionMode === "free") return;
    const pathAccessMode = config.pathAccess.mode === "block" ? "block" : "ask";

    const bashCommand = event.toolName === "bash" ? String(input.command ?? "") : undefined;
    const canonicalCwd = await canonicalizeFromCwd(ctx.cwd, ctx.cwd);
    const acceptedGrants: PendingPathGrant[] = [];
    const addedDirectoryRoots: AllowedPath[] = [...workspace.paths].map((path) => ({
      kind: "directory",
      path,
    }));

    for (const target of targets) {
      if (!target.checkPathAccess) continue;
      const absolutePath = target.path;
      const action = {
        kind: "file" as const,
        path: absolutePath,
        origin: event.toolName,
      };
      const state: PathAccessState = {
        cwd: canonicalCwd,
        mode: pathAccessMode,
        allowedPaths: [
          ...resolveAllowedPaths(config.pathAccess.allowedPaths, canonicalCwd),
          ...builtInAllowedPaths,
          ...currentSkillAllowedPaths,
          ...addedDirectoryRoots,
          ...pendingAllowedPaths(acceptedGrants),
        ],
        hasUI: ctx.hasUI,
      };
      const safety = await checkAction(action, [createPathAccessRule(state)]);
      if (safety.kind === "safe") continue;

      if (pathAccessMode === "block" || !ctx.hasUI) {
        return { block: true, reason: safety.reason };
      }

      const parentDir = dirname(absolutePath);
      const showFileOptions = event.toolName !== "ls" && event.toolName !== "find";
      const promptOpened = createPromptOpenedPayload({
        feature: "pathAccess",
        action: safety.action,
        reason: safety.reason,
        prompt: {
          kind: "confirmation",
          metadata: safety.metadata,
        },
        context: { toolName: event.toolName, input },
      });
      pi.events.emit(GUARDRAILS_PROMPT_OPENED_EVENT, promptOpened);

      let result: PromptResult | undefined;
      try {
        result = await ctx.ui.custom<PromptResult>(
          createPathAccessPromptComponent(
            event.toolName,
            safety.metadata.displayPath,
            normalizeForDisplay(parentDir, ctx.cwd),
            ctx.cwd,
            showFileOptions,
            bashCommand,
            ctx.isProjectTrusted(),
          ),
        );
      } finally {
        pi.events.emit(GUARDRAILS_PROMPT_CLOSED_EVENT, createPromptClosedPayload(promptOpened));
      }

      if (result === "allow-file-once" || result === "allow-dir-once") {
        continue;
      }

      if (result === "allow-file-session" || result === "allow-file-always") {
        const grant = createPendingGrant(
          absolutePath,
          false,
          result === "allow-file-session" ? "memory" : "local",
        );
        acceptedGrants.push(grant);
        await persistGrant(grant);
        continue;
      }

      if (result === "allow-dir-session" || result === "allow-dir-always") {
        const dirPath = showFileOptions ? parentDir : absolutePath;
        if (isGrantTooBroad(dirPath)) {
          ctx.ui.notify(
            `Cannot add ${normalizeForDisplay(dirPath, ctx.cwd)}/ as a workspace root — too broad. Treating as allow once.`,
            "warning",
          );
          continue;
        }
        const grant = createPendingGrant(dirPath, true, "memory");
        acceptedGrants.push(grant);
        await workspace.add(dirPath, result === "allow-dir-session" ? "session" : "project", ctx);
        continue;
      }

      const reason = "User denied access outside working directory";
      return { block: true, reason };
    }
  });
}
