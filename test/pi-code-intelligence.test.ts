import assert from "node:assert/strict";
import test from "node:test";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import codeIntelligence, {
  NATIVE_TOOL_LIMIT,
  REMINDER_TYPE,
  ROUTING_GUIDANCE,
} from "../plugins/pi-code-intelligence/src/index.ts";

type Handler = (event: unknown) => unknown;
type SentMessage = {
  message: { customType?: string; content?: string; display?: boolean };
  options: { deliverAs?: string };
};

function harness() {
  const handlers = new Map<string, Handler>();
  const messages: SentMessage[] = [];
  codeIntelligence({
    on(event: string, handler: Handler) {
      handlers.set(event, handler);
    },
    sendMessage(message: SentMessage["message"], options: SentMessage["options"]) {
      messages.push({ message, options });
    },
  } as unknown as ExtensionAPI);

  const call = (event: string, input: unknown) => handlers.get(event)?.(input);
  return { call, messages };
}

test("adds stable code-intelligence routing guidance", () => {
  const { call } = harness();
  const result = call("before_agent_start", { systemPrompt: "base" }) as {
    systemPrompt: string;
  };

  assert.equal(result.systemPrompt, `base\n\n${ROUTING_GUIDANCE}`);
});

test("nudges after four native searches without blocking them", () => {
  const { call, messages } = harness();

  for (let index = 0; index < NATIVE_TOOL_LIMIT; index += 1) {
    assert.equal(
      call("tool_call", { toolName: index % 2 === 0 ? "read" : "grep", input: {} }),
      undefined,
    );
  }

  assert.equal(messages.length, 1);
  assert.equal(messages[0]?.message.customType, REMINDER_TYPE);
  assert.match(messages[0]?.message.content ?? "", /Unless the user requested native-only work/);
  assert.equal(messages[0]?.message.display, false);
  assert.deepEqual(messages[0]?.options, { deliverAs: "steer" });

  call("tool_call", { toolName: "read", input: {} });
  assert.equal(messages.length, 1);
});

test("code-intelligence use clears the reminder and permits a later nudge", () => {
  const { call, messages } = harness();
  const reminder = { role: "custom", customType: REMINDER_TYPE, content: "old" };

  for (let index = 0; index < NATIVE_TOOL_LIMIT; index += 1) {
    call("tool_call", { toolName: "read", input: {} });
  }
  assert.deepEqual(call("context", { messages: [reminder] }), undefined);

  call("tool_call", { toolName: "mcp__serena", input: {} });
  assert.deepEqual(call("context", { messages: [reminder, { role: "user", content: "task" }] }), {
    messages: [{ role: "user", content: "task" }],
  });

  for (let index = 0; index < NATIVE_TOOL_LIMIT; index += 1) {
    call("tool_call", { toolName: "grep", input: {} });
  }
  assert.equal(messages.length, 2);
});

test("gateway calls detect both code-intelligence servers", () => {
  for (const input of [
    { server: "serena" },
    { tool: "codebase-memory-mcp/query" },
    { code: 'tools.call("codebase_memory_mcp/search", {})' },
  ]) {
    const { call, messages } = harness();
    for (let index = 1; index < NATIVE_TOOL_LIMIT; index += 1) {
      call("tool_call", { toolName: "read", input: {} });
    }
    call("tool_call", { toolName: "mcp", input });
    call("tool_call", { toolName: "read", input: {} });
    assert.equal(messages.length, 0);
  }
});
