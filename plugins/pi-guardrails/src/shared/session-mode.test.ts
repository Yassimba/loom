import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ResolvedConfig } from "./config";
import {
  configuredSessionMode,
  cycleSessionMode,
  enableYoloMode,
  getSessionMode,
  resetSessionMode,
} from "./session-mode";

function config(overrides: Partial<ResolvedConfig> = {}): ResolvedConfig {
  return {
    version: "test",
    enabled: true,
    applyBuiltinDefaults: true,
    features: { policies: true, permissionGate: true, pathAccess: true },
    policies: { rules: [] },
    pathAccess: { mode: "ask", allowedPaths: [], workspaceRoots: [] },
    permissionGate: {
      patterns: [],
      useBuiltinMatchers: true,
      requireConfirmation: true,
      allowedPatterns: [],
      autoDenyPatterns: [],
    },
    ...overrides,
    modeShortcut: overrides.modeShortcut ?? "ctrl+alt+g",
  };
}

describe("Guardrails session mode", () => {
  beforeEach(() => resetSessionMode("ask"));

  it("uses configured safety as the initial session mode", () => {
    expect(configuredSessionMode(config())).toBe("ask");
    expect(
      configuredSessionMode(
        config({ features: { policies: true, permissionGate: true, pathAccess: false } }),
      ),
    ).toBe("free");
    expect(configuredSessionMode(config({ enabled: false }))).toBe("yolo");
  });

  it("enables Yolo directly after confirmation", async () => {
    const confirm = vi.fn().mockResolvedValueOnce(false).mockResolvedValue(true);

    await enableYoloMode(confirm);
    expect(getSessionMode()).toBe("ask");

    await enableYoloMode(confirm);
    expect(getSessionMode()).toBe("yolo");

    await enableYoloMode(confirm);
    expect(confirm).toHaveBeenCalledTimes(2);
  });

  it("cycles modes and confirms Yolo once per session", async () => {
    const confirm = vi.fn().mockResolvedValueOnce(false).mockResolvedValue(true);

    await cycleSessionMode(confirm);
    expect(getSessionMode()).toBe("free");

    await cycleSessionMode(confirm);
    expect(getSessionMode()).toBe("free");

    await cycleSessionMode(confirm);
    expect(getSessionMode()).toBe("yolo");
    await cycleSessionMode(confirm);
    await cycleSessionMode(confirm);
    await cycleSessionMode(confirm);

    expect(getSessionMode()).toBe("yolo");
    expect(confirm).toHaveBeenCalledTimes(2);
  });
});
