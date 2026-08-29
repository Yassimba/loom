import assert from "node:assert/strict";
import { test } from "node:test";
import type { ExtensionAPI, ExtensionCommandContext } from "@earendil-works/pi-coding-agent";
import {
  registerSandboxBridge,
  SANDBOX_GRANT_EVENT,
} from "../plugins/pi-sandbox-bridge-prototype/index.ts";

test("the bridge grants the requested scope without showing the sandbox prompt", async () => {
  const handlers = new Map<string, (data: unknown) => void>();
  let commandArgs: string | undefined;
  let promptResult: unknown;
  const pi = {
    events: {
      emit(channel: string, data: unknown) {
        handlers.get(channel)?.(data);
      },
      on(channel: string, handler: (data: unknown) => void) {
        handlers.set(channel, handler);
        return () => handlers.delete(channel);
      },
    },
    registerCommand() {},
  } as unknown as ExtensionAPI;

  registerSandboxBridge(pi, (sandboxPi) => {
    sandboxPi.registerCommand("sandbox-allow", {
      description: "test seam",
      handler: async (args, ctx) => {
        commandArgs = args;
        promptResult = await ctx.ui.custom(() => ({ render: () => [], invalidate() {} }));
      },
    });
  });

  const ctx = {
    ui: {
      custom: async () => {
        throw new Error("The original sandbox prompt was shown");
      },
      notify() {},
    },
  } as unknown as ExtensionCommandContext;
  let accepted = false;
  await new Promise<void>((resolve, reject) => {
    pi.events.emit(SANDBOX_GRANT_EVENT, {
      access: "write",
      path: "/external/project",
      scope: "session",
      ctx,
      accept: () => {
        accepted = true;
      },
      resolve,
      reject,
    });
  });

  assert.equal(accepted, true);
  assert.equal(commandArgs, "write /external/project");
  assert.deepEqual(promptResult, { action: "session", value: "/external/project" });
});
