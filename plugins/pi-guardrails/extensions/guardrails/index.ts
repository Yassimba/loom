import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";
import { configLoader } from "../../src/shared/config";
import {
  configuredSessionMode,
  cycleSessionMode,
  enableYoloMode,
  GUARDRAILS_MODE_CHANGED_EVENT,
  getSessionMode,
  resetSessionMode,
} from "../../src/shared/session-mode";
import { registerPathAccess } from "../path-access";
import { registerAddDir } from "./commands/add-dir";
import { registerGuardrailsExamplesCommand } from "./commands/examples";
import { registerGuardrailsSettings } from "./commands/settings";

const STATUS_LABELS = { ask: "Ask", free: "Free", yolo: "Yolo" } as const;

function publishMode(pi: ExtensionAPI, ctx: ExtensionContext): void {
  const mode = getSessionMode();
  const color = mode === "ask" ? "muted" : mode === "free" ? "warning" : "error";
  ctx.ui.setStatus("guardrails-mode", ctx.ui.theme.fg(color, STATUS_LABELS[mode]));
  pi.events.emit(GUARDRAILS_MODE_CHANGED_EVENT, { mode });
}

function confirmYolo(ctx: ExtensionContext): Promise<boolean> {
  return ctx.hasUI
    ? ctx.ui.confirm(
        "Enable Yolo mode?",
        "All Guardrails checks will be disabled for this Pi session.",
      )
    : Promise.resolve(false);
}

async function cycleMode(pi: ExtensionAPI, ctx: ExtensionContext): Promise<void> {
  await cycleSessionMode(() => confirmYolo(ctx));
  publishMode(pi, ctx);
}

async function enableYolo(pi: ExtensionAPI, ctx: ExtensionContext): Promise<void> {
  await enableYoloMode(() => confirmYolo(ctx));
  publishMode(pi, ctx);
}

export default async function guardrails(pi: ExtensionAPI) {
  await configLoader.load();
  const workspace = registerAddDir(pi);
  registerPathAccess(pi, workspace);
  registerGuardrailsSettings(pi);
  registerGuardrailsExamplesCommand(pi);

  pi.registerCommand("guardrails:mode", {
    description: "Cycle Guardrails session mode: Ask, Free, Yolo",
    handler: async (_args, ctx) => cycleMode(pi, ctx),
  });
  pi.registerCommand("yolo", {
    description: "Disable all Guardrails checks for this session",
    handler: async (_args, ctx) => enableYolo(pi, ctx),
  });

  const shortcut = configLoader.getConfig().modeShortcut;
  if (shortcut !== "disabled") {
    pi.registerShortcut(shortcut, {
      description: "Cycle Guardrails session mode",
      handler: (ctx) => cycleMode(pi, ctx),
    });
  }

  pi.on("session_start", (_event, ctx) => {
    resetSessionMode(configuredSessionMode(configLoader.getConfig()));
    publishMode(pi, ctx);

    const warnings = configLoader.drainMessages();
    if (warnings.length === 1) {
      ctx.ui.notify(warnings[0], "warning");
    } else if (warnings.length > 1) {
      ctx.ui.notify(
        ["Guardrails warnings:", ...warnings.map((warning) => `- ${warning}`)].join("\n"),
        "warning",
      );
    }
  });
}
