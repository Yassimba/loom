import { describe, expect, it } from "vitest";
import { createPromptClosedPayload, createPromptOpenedPayload } from "./events";

const promptEvent = {
  feature: "permissionGate" as const,
  action: {
    kind: "command" as const,
    command: "dangerous-cmd",
    origin: "bash",
  },
  reason: "test danger",
  prompt: {
    kind: "permission" as const,
    metadata: { pattern: "dangerous-cmd" },
  },
};

describe("prompt event payloads", () => {
  it("creates correlated opened and closed payloads", () => {
    const opened = createPromptOpenedPayload(promptEvent);
    const closed = createPromptClosedPayload(opened);

    expect(closed).toEqual(
      expect.objectContaining({
        source: "guardrails",
        feature: "permissionGate",
        prompt: { id: opened.prompt.id },
      }),
    );
  });

  it("creates a unique ID for each prompt", () => {
    const first = createPromptOpenedPayload(promptEvent);
    const second = createPromptOpenedPayload(promptEvent);

    expect(first.prompt.id).not.toBe(second.prompt.id);
  });
});
