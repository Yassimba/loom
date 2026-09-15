import type {
  BashToolCallEvent,
  ExtensionAPI,
  ExtensionContext,
  ExtensionHandler,
  ReadToolCallEvent,
  ToolCallEvent,
  ToolCallEventResult,
} from "@earendil-works/pi-coding-agent";
import { createMock, type DeepMocked, type PartialFuncReturn } from "@golevelup/ts-vitest";
import { assert, beforeEach, describe, expect, it, vi } from "vitest";
import {
  GUARDRAILS_PROMPT_CLOSED_EVENT,
  GUARDRAILS_PROMPT_OPENED_EVENT,
  type GuardrailsPromptOpenedPayload,
} from "../../src/shared/events";
import {
  GUARDRAILS_MODE_CHANGED_EVENT,
  type GuardrailsModeChangedPayload,
} from "../../src/shared/session-mode";
import permissionGate from "./index";

// Control the config the hook sees without touching the real config loader.
vi.mock("../../src/shared/config", () => {
  function makeConfig(overrides: Record<string, unknown> = {}) {
    return {
      enabled: true,
      features: { permissionGate: true, policies: true, pathAccess: true },
      permissionGate: {
        patterns: [{ pattern: "dangerous-cmd", description: "test danger" }],
        useBuiltinMatchers: false,
        requireConfirmation: true,
        allowedPatterns: [],
        autoDenyPatterns: [],
        ...overrides,
      },
    };
  }

  return {
    configLoader: {
      load: vi.fn().mockResolvedValue(undefined),
      getConfig: vi.fn(() => makeConfig()),
    },
  };
});

type ToolCallHandler = ExtensionHandler<ToolCallEvent, ToolCallEventResult>;

function registeredToolCallHandler(pi: DeepMocked<ExtensionAPI>) {
  const calls: unknown[][] = pi.on.mock.calls;
  return calls.find(([event]) => event === "tool_call")?.[1] as ToolCallHandler | undefined;
}

function setMode(pi: DeepMocked<ExtensionAPI>, mode: GuardrailsModeChangedPayload["mode"]): void {
  const calls: unknown[][] = pi.events.on.mock.calls;
  const handler = calls.find(([event]) => event === GUARDRAILS_MODE_CHANGED_EVENT)?.[1];
  assert(typeof handler === "function", "mode change handler should be registered");
  handler({ mode });
}

function createCtx(overrides: PartialFuncReturn<ExtensionContext> = {}) {
  return createMock<ExtensionContext>({
    hasUI: true,
    mode: "tui",
    ui: {
      custom: vi.fn().mockResolvedValue(undefined),
      select: vi.fn().mockResolvedValue(undefined),
      notify: vi.fn(),
    },
    abort: vi.fn(),
    ...overrides,
  });
}

const DANGEROUS_EVENT = {
  type: "tool_call",
  toolCallId: "dangerous-call",
  toolName: "bash",
  input: { command: "dangerous-cmd" },
} satisfies BashToolCallEvent;

describe("permissionGate extension hook", () => {
  let pi: DeepMocked<ExtensionAPI>;
  let toolCallHandler: ToolCallHandler | undefined;

  beforeEach(async () => {
    pi = createMock<ExtensionAPI>();
    await permissionGate(pi);
    toolCallHandler = registeredToolCallHandler(pi);
  });

  it("returns undefined for safe commands", async () => {
    assert(toolCallHandler, "tool_call handler should be registered");

    const result = await toolCallHandler(
      {
        type: "tool_call",
        toolCallId: "safe-call",
        toolName: "bash",
        input: { command: "echo hello" },
      } satisfies BashToolCallEvent,
      createCtx(),
    );
    expect(result).toBeUndefined();
  });

  it("returns undefined for non-bash tools", async () => {
    assert(toolCallHandler, "tool_call handler should be registered");

    const result = await toolCallHandler(
      {
        type: "tool_call",
        toolCallId: "read-call",
        toolName: "read",
        input: { path: "dangerous-cmd" },
      } satisfies ReadToolCallEvent,
      createCtx(),
    );
    expect(result).toBeUndefined();
  });

  it("bypasses dangerous-command checks in Yolo mode", async () => {
    assert(toolCallHandler, "tool_call handler should be registered");
    setMode(pi, "yolo");
    const ctx = createCtx();

    expect(await toolCallHandler(DANGEROUS_EVENT, ctx)).toBeUndefined();
    expect(ctx.ui.custom).not.toHaveBeenCalled();
  });

  it("deny returns { block: true } without aborting the turn", async () => {
    assert(toolCallHandler, "tool_call handler should be registered");

    const ctx = createCtx({
      ui: { custom: vi.fn().mockResolvedValue("deny"), select: vi.fn() },
    });

    const result = await toolCallHandler(DANGEROUS_EVENT, ctx);

    expect(result).toEqual({
      block: true,
      reason: "User denied dangerous command",
    });
    expect(ctx.abort).not.toHaveBeenCalled();
  });

  it("stop calls ctx.abort() and returns { block: true }", async () => {
    assert(toolCallHandler, "tool_call handler should be registered");

    const ctx = createCtx({
      ui: { custom: vi.fn().mockResolvedValue("stop"), select: vi.fn() },
    });

    const result = await toolCallHandler(DANGEROUS_EVENT, ctx);

    expect(result).toEqual({
      block: true,
      reason: "User declined and stopped dangerous command",
    });
    expect(ctx.abort).toHaveBeenCalledTimes(1);
  });

  it("allow once returns undefined and does not abort", async () => {
    assert(toolCallHandler, "tool_call handler should be registered");

    const ctx = createCtx({
      ui: { custom: vi.fn().mockResolvedValue("allow"), select: vi.fn() },
    });

    const result = await toolCallHandler(DANGEROUS_EVENT, ctx);
    expect(result).toBeUndefined();
    expect(ctx.abort).not.toHaveBeenCalled();

    const opened = pi.events.emit.mock.calls.find(
      ([event]) => event === GUARDRAILS_PROMPT_OPENED_EVENT,
    )?.[1] as GuardrailsPromptOpenedPayload | undefined;
    assert(opened, "prompt opened event should be emitted");
    expect(pi).toHaveEmitted(
      GUARDRAILS_PROMPT_CLOSED_EVENT,
      expect.objectContaining({ prompt: { id: opened.prompt.id } }),
    );
  });

  it("closes the prompt when its UI throws", async () => {
    assert(toolCallHandler, "tool_call handler should be registered");

    const ctx = createCtx({
      ui: {
        custom: vi.fn().mockRejectedValue(new Error("UI failed")),
        select: vi.fn(),
      },
    });

    await expect(toolCallHandler(DANGEROUS_EVENT, ctx)).rejects.toThrow("UI failed");

    const opened = pi.events.emit.mock.calls.find(
      ([event]) => event === GUARDRAILS_PROMPT_OPENED_EVENT,
    )?.[1] as GuardrailsPromptOpenedPayload | undefined;
    assert(opened, "prompt opened event should be emitted");
    expect(pi).toHaveEmitted(
      GUARDRAILS_PROMPT_CLOSED_EVENT,
      expect.objectContaining({ prompt: { id: opened.prompt.id } }),
    );
  });

  it("RPC fallback exposes 'Decline and stop' and maps it to stop", async () => {
    assert(toolCallHandler, "tool_call handler should be registered");

    const select = vi.fn().mockResolvedValue("Decline and stop");
    const ctx = createCtx({
      ui: { custom: vi.fn().mockResolvedValue(undefined), select },
    });

    const result = await toolCallHandler(DANGEROUS_EVENT, ctx);

    expect(select).toHaveBeenCalledWith(
      expect.stringContaining("test danger"),
      expect.arrayContaining(["Allow once", "Allow for session", "Deny", "Decline and stop"]),
    );
    expect(result).toEqual({
      block: true,
      reason: "User declined and stopped dangerous command",
    });
    expect(ctx.abort).toHaveBeenCalledTimes(1);
  });

  it("non-interactive (no UI) blocks and does not abort", async () => {
    assert(toolCallHandler, "tool_call handler should be registered");

    const ctx = createCtx({ hasUI: false });
    const result = await toolCallHandler(DANGEROUS_EVENT, ctx);

    expect(result).toEqual({
      block: true,
      reason: expect.stringContaining("no UI to confirm"),
    });
    expect(ctx.abort).not.toHaveBeenCalled();
  });
});
