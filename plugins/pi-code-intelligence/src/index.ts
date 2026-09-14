import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

export const NATIVE_TOOL_LIMIT = 4;
export const REMINDER_TYPE = "code-intelligence-reminder";
export const ROUTING_GUIDANCE = `Code intelligence routing:
- For architecture, call paths, and change impact, use Codebase Memory before tracing manually when indexed.
- Verify important findings with source reads, grep, and tests.`;
export const EXPLORATION_REMINDER =
  "Pause native exploration. Unless the user requested native-only work, query Codebase Memory for architecture and impact before another read or grep. Then verify the result with native tools.";

function isCodeIntelligenceCall(toolName: string, input: unknown): boolean {
  if (toolName === "mcp__codebase_memory_mcp") return true;
  if (toolName !== "mcp" && toolName !== "mcpScript") return false;
  const call = JSON.stringify(input)?.toLowerCase() ?? "";
  return call.includes("codebase-memory") || call.includes("codebase_memory");
}

export default function codeIntelligence(pi: ExtensionAPI): void {
  let nativeToolUses = 0;
  let reminderActive = false;

  pi.on("session_start", () => {
    nativeToolUses = 0;
    reminderActive = false;
  });

  pi.on("before_agent_start", (event) => ({
    systemPrompt: `${event.systemPrompt}\n\n${ROUTING_GUIDANCE}`,
  }));

  pi.on("context", (event) => {
    if (reminderActive) return;
    return {
      messages: event.messages.filter(
        (message) =>
          (message as typeof message & { customType?: string }).customType !== REMINDER_TYPE,
      ),
    };
  });

  pi.on("tool_call", (event) => {
    if (isCodeIntelligenceCall(event.toolName, event.input)) {
      nativeToolUses = 0;
      reminderActive = false;
      return;
    }
    if (reminderActive || (event.toolName !== "grep" && event.toolName !== "read")) return;

    nativeToolUses += 1;
    if (nativeToolUses < NATIVE_TOOL_LIMIT) return;

    reminderActive = true;
    pi.sendMessage(
      {
        customType: REMINDER_TYPE,
        content: EXPLORATION_REMINDER,
        display: false,
      },
      { deliverAs: "steer" },
    );
  });
}
