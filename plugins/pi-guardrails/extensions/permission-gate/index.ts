import { type ExtensionAPI, isToolCallEventType } from "@earendil-works/pi-coding-agent";
import { checkAction } from "../../src/core";
import { configLoader } from "../../src/shared/config";
import {
  createPromptClosedPayload,
  createPromptOpenedPayload,
  GUARDRAILS_PROMPT_CLOSED_EVENT,
  GUARDRAILS_PROMPT_OPENED_EVENT,
} from "../../src/shared/events";
import { isCommandAllowed, saveCommandSessionGrant } from "./grants";
import { createPermissionGateConfirmComponent } from "./prompt";
import { createPermissionGateRule, formatAutoDenyReason, matchCommandPattern } from "./rules";

export default async function permissionGate(pi: ExtensionAPI) {
  await configLoader.load();

  pi.on("tool_call", async (event, ctx) => {
    const config = configLoader.getConfig();
    if (!config.enabled || !config.features.permissionGate) return;
    if (!isToolCallEventType("bash", event)) return;

    const command = event.input.command;
    const action = { kind: "command" as const, command, origin: "bash" };
    if (isCommandAllowed(command)) return;

    const autoDenyMatch = matchCommandPattern(command, config.permissionGate.autoDenyPatterns);

    if (autoDenyMatch) {
      const reason = formatAutoDenyReason(autoDenyMatch);

      return { block: true, reason };
    }

    const safety = await checkAction(action, [
      createPermissionGateRule({
        patterns: config.permissionGate.patterns,
        useBuiltinMatchers: config.permissionGate.useBuiltinMatchers,
      }),
    ]);
    if (safety.kind === "safe") return;

    if (!config.permissionGate.requireConfirmation) {
      ctx.ui.notify(`Dangerous command detected: ${safety.reason}`, "warning");
      return;
    }

    if (!ctx.hasUI) {
      const reason = `Dangerous command blocked (no UI to confirm): ${safety.reason}`;
      return { block: true, reason };
    }

    type ConfirmResult = "allow" | "allow-session" | "deny" | "stop";
    const promptOpened = createPromptOpenedPayload({
      feature: "permissionGate",
      action: safety.action,
      reason: safety.reason,
      prompt: {
        kind: "permission",
        metadata: safety.metadata,
      },
      context: { toolName: "bash", input: event.input },
    });
    pi.events.emit(GUARDRAILS_PROMPT_OPENED_EVENT, promptOpened);

    let result: ConfirmResult;
    try {
      const customResult = await ctx.ui.custom<ConfirmResult>(
        createPermissionGateConfirmComponent(command, safety.reason),
      );

      if (customResult === undefined) {
        const selection = await ctx.ui.select(`Dangerous command: ${safety.reason}`, [
          "Allow once",
          "Allow for session",
          "Deny",
          "Decline and stop",
        ]);
        if (selection === "Allow once") result = "allow";
        else if (selection === "Allow for session") result = "allow-session";
        else if (selection === "Decline and stop") result = "stop";
        else result = "deny";
      } else {
        result = customResult;
      }
    } finally {
      pi.events.emit(GUARDRAILS_PROMPT_CLOSED_EVENT, createPromptClosedPayload(promptOpened));
    }

    if (result === "allow") return;
    if (result === "allow-session") {
      await saveCommandSessionGrant(command);
      return;
    }

    if (result === "stop") {
      const reason = "User declined and stopped dangerous command";
      ctx.abort();
      return { block: true, reason };
    }

    const reason = "User denied dangerous command";
    return { block: true, reason };
  });
}
