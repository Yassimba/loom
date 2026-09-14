import { buildSchemaUrl, ConfigLoader, type Scope } from "@aliou/pi-utils-settings";
import pkg from "../../../package.json" with { type: "json" };
import type { AllowedPath } from "../../core/paths/path";
import { DEFAULT_CONFIG } from "./defaults";
import { migrations } from "./migration";
import type { GuardrailsConfig, PolicyRule, ResolvedConfig, WorkspaceRoot } from "./types";

class GuardrailsConfigLoader extends ConfigLoader<GuardrailsConfig, ResolvedConfig> {
  override async save(scope: Scope, config: GuardrailsConfig): Promise<void> {
    await super.save(scope, ensureConfigVersion(config));
  }
}

function ensureConfigVersion(config: GuardrailsConfig): GuardrailsConfig {
  if (typeof config.version === "string" && config.version.trim()) {
    return config;
  }
  return { ...config, version: pkg.version };
}

function mergePolicies(
  applyBuiltinDefaults: boolean,
  ...ruleGroups: Array<PolicyRule[] | undefined>
): PolicyRule[] {
  const rules = new Map<string, PolicyRule>();
  for (const rule of applyBuiltinDefaults ? DEFAULT_CONFIG.policies.rules : [])
    rules.set(rule.id, rule);
  for (const group of ruleGroups) {
    for (const rule of group ?? []) rules.set(rule.id, rule);
  }
  return [...rules.values()];
}

function mergeAllowedPaths(...pathGroups: Array<AllowedPath[] | undefined>): AllowedPath[] {
  const paths = new Map<string, AllowedPath>();
  for (const group of pathGroups) {
    for (const entry of group ?? []) {
      if (!entry || typeof entry !== "object") continue;
      const path = typeof entry.path === "string" ? entry.path.trim() : "";
      if (!path) continue;
      const kind = entry.kind === "directory" ? "directory" : "file";
      paths.set(`${kind}:${path}`, { kind, path });
    }
  }
  return [...paths.values()];
}

function mergeWorkspaceRoots(...rootGroups: Array<WorkspaceRoot[] | undefined>): WorkspaceRoot[] {
  const roots = new Map<string, WorkspaceRoot>();
  for (const group of rootGroups) {
    for (const root of group ?? []) {
      if (!root || typeof root !== "object" || typeof root.path !== "string") continue;
      const path = root.path.trim();
      if (!path) continue;
      roots.set(path, { path, orientation: root.orientation ?? roots.get(path)?.orientation });
    }
  }
  return [...roots.values()];
}

export function createGuardrailsConfigLoader(): GuardrailsConfigLoader {
  return new GuardrailsConfigLoader("guardrails", DEFAULT_CONFIG, {
    scopes: ["global", "local", "memory"],
    migrations,
    schemaUrl: buildSchemaUrl(pkg.name, pkg.version),
    afterMerge: (resolved, global, local, memory) => {
      resolved.policies.rules = mergePolicies(
        resolved.applyBuiltinDefaults,
        global?.policies?.rules,
        local?.policies?.rules,
        memory?.policies?.rules,
      );

      const customPatterns =
        memory?.permissionGate?.customPatterns ??
        local?.permissionGate?.customPatterns ??
        global?.permissionGate?.customPatterns;
      if (customPatterns) {
        resolved.permissionGate.patterns = customPatterns;
        resolved.permissionGate.useBuiltinMatchers = false;
      }

      resolved.pathAccess.allowedPaths = mergeAllowedPaths(
        global?.pathAccess?.allowedPaths,
        local?.pathAccess?.allowedPaths,
        memory?.pathAccess?.allowedPaths,
      );
      resolved.pathAccess.workspaceRoots = mergeWorkspaceRoots(
        global?.pathAccess?.workspaceRoots,
        local?.pathAccess?.workspaceRoots,
        memory?.pathAccess?.workspaceRoots,
      );

      return resolved;
    },
  });
}

export const configLoader = createGuardrailsConfigLoader();
