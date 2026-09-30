import type {
  BashToolCallEvent,
  ExtensionAPI,
  ExtensionContext,
  ReadToolCallEvent,
} from "@earendil-works/pi-coding-agent";
import { createMock, type DeepMocked } from "@golevelup/ts-vitest";
import { assert, beforeEach, describe, expect, it, vi } from "vitest";
import { configLoader } from "../../src/shared/config";
import {
  GUARDRAILS_PROMPT_CLOSED_EVENT,
  GUARDRAILS_PROMPT_OPENED_EVENT,
  type GuardrailsPromptOpenedPayload,
} from "../../src/shared/events";
import { resetSessionMode } from "../../src/shared/session-mode";
import type { WorkspaceRootControl } from "../guardrails/commands/add-dir";
import { registerPathAccess } from "./index";
import type { createPathAccessPromptComponent } from "./prompt";
import { targetsForTool } from "./targets";

vi.mock("../../src/shared/config", () => ({
  configLoader: {
    load: vi.fn(async () => undefined),
    getConfig: vi.fn(() => ({
      enabled: true,
      features: { pathAccess: true },
      pathAccess: { mode: "ask", allowedPaths: [] },
    })),
  },
}));

vi.mock("./dynamic-resources", () => ({
  piDocumentationPaths: vi.fn(() => []),
  temporaryPaths: vi.fn(() => []),
}));

vi.mock("./targets", () => ({
  targetsForTool: vi.fn(async () => [
    { path: "/outside/secret.txt", unresolved: false, checkPathAccess: true },
  ]),
}));

const toolCall = {
  type: "tool_call",
  toolCallId: "test-call",
  toolName: "read",
  input: { path: "/outside/secret.txt" },
} satisfies ReadToolCallEvent;

const bashToolCall = {
  type: "tool_call",
  toolCallId: "test-call",
  toolName: "bash",
  input: { command: "cat /outside/secret.txt" },
} satisfies BashToolCallEvent;

type PathAccessPromptComponent = ReturnType<typeof createPathAccessPromptComponent>;

const theme = {
  fg: (_color: string, text: string) => text,
  bg: (_color: string, text: string) => text,
  bold: (text: string) => text,
};

function renderPromptComponent(component: PathAccessPromptComponent): string {
  return component(
    { terminal: { columns: 100 }, requestRender: vi.fn() },
    theme,
    undefined,
    vi.fn(),
  )
    .render(100)
    .join("\n");
}

function emittedPromptOpened(pi: DeepMocked<ExtensionAPI>) {
  return pi.events.emit.mock.calls.find(
    ([event]) => event === GUARDRAILS_PROMPT_OPENED_EVENT,
  )?.[1] as GuardrailsPromptOpenedPayload | undefined;
}

function workspace(paths: string[] = []): WorkspaceRootControl {
  return { paths: new Set(paths), add: vi.fn(async () => true) };
}

function registeredExtensionHandler(pi: DeepMocked<ExtensionAPI>, event: string) {
  const calls: unknown[][] = pi.on.mock.calls;
  return calls.find(([registeredEvent]) => registeredEvent === event)?.[1];
}

describe("pathAccess extension hook", () => {
  beforeEach(() => resetSessionMode("ask"));

  it("emits a correlated lifecycle around an outside-path prompt", async () => {
    const pi = createMock<ExtensionAPI>();
    const ctx = createMock<ExtensionContext>({
      cwd: "/workspace",
      hasUI: true,
      mode: "tui",
    });
    ctx.ui.custom.mockResolvedValue("allow-file-once");
    registerPathAccess(pi, workspace());

    const toolCallHandler = registeredExtensionHandler(pi, "tool_call");
    assert(typeof toolCallHandler === "function", "tool_call handler should be registered");
    await toolCallHandler(toolCall, ctx);

    const opened = emittedPromptOpened(pi);
    assert(opened, "prompt opened event should be emitted");
    expect(pi).toHaveEmitted(
      GUARDRAILS_PROMPT_CLOSED_EVENT,
      expect.objectContaining({ prompt: { id: opened.prompt.id } }),
    );
  });

  it("closes the prompt when its UI throws", async () => {
    const pi = createMock<ExtensionAPI>();
    const ctx = createMock<ExtensionContext>({
      cwd: "/workspace",
      hasUI: true,
      mode: "tui",
    });
    ctx.ui.custom.mockRejectedValue(new Error("UI failed"));
    registerPathAccess(pi, workspace());

    const toolCallHandler = registeredExtensionHandler(pi, "tool_call");
    assert(typeof toolCallHandler === "function", "tool_call handler should be registered");
    await expect(toolCallHandler(toolCall, ctx)).rejects.toThrow("UI failed");

    const opened = emittedPromptOpened(pi);
    assert(opened, "prompt opened event should be emitted");
    expect(pi).toHaveEmitted(
      GUARDRAILS_PROMPT_CLOSED_EVENT,
      expect.objectContaining({ prompt: { id: opened.prompt.id } }),
    );
  });

  it("blocks protected files from the shared target pipeline", async () => {
    vi.mocked(targetsForTool).mockResolvedValueOnce([
      { path: "/outside/secret.txt", unresolved: false, checkPathAccess: false },
    ]);
    vi.mocked(configLoader.getConfig).mockReturnValueOnce({
      enabled: true,
      features: { policies: true, pathAccess: false, permissionGate: true },
      policies: {
        rules: [
          {
            id: "secret",
            patterns: [{ pattern: "/outside/secret.txt" }],
            protection: "noAccess",
            onlyIfExists: false,
          },
        ],
      },
      pathAccess: { mode: "ask", allowedPaths: [], workspaceRoots: [] },
      permissionGate: {
        patterns: [],
        useBuiltinMatchers: true,
        requireConfirmation: true,
        allowedPatterns: [],
        autoDenyPatterns: [],
      },
      version: "0.17.1",
      applyBuiltinDefaults: true,
      modeShortcut: "ctrl+alt+g",
    });
    const pi = createMock<ExtensionAPI>();
    const ctx = createMock<ExtensionContext>({ cwd: "/workspace", hasUI: true, mode: "tui" });
    registerPathAccess(pi, workspace());

    const toolCallHandler = registeredExtensionHandler(pi, "tool_call");
    assert(typeof toolCallHandler === "function", "tool_call handler should be registered");
    await expect(toolCallHandler(toolCall, ctx)).resolves.toMatchObject({ block: true });
    expect(ctx.ui.custom).not.toHaveBeenCalled();
  });

  it("skips outside-path prompts in Free mode", async () => {
    resetSessionMode("free");
    const pi = createMock<ExtensionAPI>();
    const ctx = createMock<ExtensionContext>({ cwd: "/workspace", hasUI: true, mode: "tui" });
    registerPathAccess(pi, workspace());

    const toolCallHandler = registeredExtensionHandler(pi, "tool_call");
    assert(typeof toolCallHandler === "function", "tool_call handler should be registered");
    await toolCallHandler(toolCall, ctx);

    expect(ctx.ui.custom).not.toHaveBeenCalled();
  });

  it("allows paths added by the bundled add-dir extension as workspace roots", async () => {
    const pi = createMock<ExtensionAPI>();
    const ctx = createMock<ExtensionContext>({
      cwd: "/workspace",
      hasUI: true,
      mode: "tui",
    });
    registerPathAccess(pi, workspace(["/outside"]));

    const toolCallHandler = registeredExtensionHandler(pi, "tool_call");
    assert(typeof toolCallHandler === "function", "tool_call handler should be registered");
    await toolCallHandler(toolCall, ctx);

    expect(ctx.ui.custom).not.toHaveBeenCalled();
  });

  it("turns a directory approval into a session workspace root", async () => {
    const pi = createMock<ExtensionAPI>();
    const ctx = createMock<ExtensionContext>({
      cwd: "/workspace",
      hasUI: true,
      mode: "tui",
      isProjectTrusted: () => true,
    });
    ctx.ui.custom.mockResolvedValue("allow-dir-session");
    const roots = workspace();
    registerPathAccess(pi, roots);

    const toolCallHandler = registeredExtensionHandler(pi, "tool_call");
    assert(typeof toolCallHandler === "function", "tool_call handler should be registered");
    await toolCallHandler(toolCall, ctx);

    expect(roots.add).toHaveBeenCalledWith("/outside", "session", ctx);
  });

  it("shows the bash command in the outside-path prompt", async () => {
    const pi = createMock<ExtensionAPI>();
    const ctx = createMock<ExtensionContext>({
      cwd: "/workspace",
      hasUI: true,
      mode: "tui",
    });
    ctx.ui.custom.mockResolvedValue("allow-file-once");
    registerPathAccess(pi, workspace());

    const toolCallHandler = registeredExtensionHandler(pi, "tool_call");
    assert(typeof toolCallHandler === "function", "tool_call handler should be registered");
    await toolCallHandler(bashToolCall, ctx);

    const promptComponent = ctx.ui.custom.mock.calls[0]?.[0] as
      | PathAccessPromptComponent
      | undefined;
    assert(promptComponent, "path access prompt should be shown");
    expect(renderPromptComponent(promptComponent)).toContain("Command: cat /outside/secret.txt");
  });
});
