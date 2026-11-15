import { expect, type TestBody, test } from "claude-code/testing";

// Capability: /better hands a prompt to a read-only helper agent and puts the
// helper's improved prompt in the prompt box.
// Seam: the engine's command.run, agent.spawn, turn.complete and prompt.fill
// events, with the test's hooks in place of the engine beneath the mod. The
// engine alone gives a spawned agent its id, so the tests that follow a
// helper's turn seed the running helper as stored state.
// Fails when: the helper is not spawned with the typed prompt, its improved
// prompt does not reach the prompt box above the draft, or a helper reply with
// no marked prompt changes the prompt box.

const HELPER_TURN = {
  agentId: "helper-1",
  reason: "answer",
  durationMs: 1,
  isAborted: false,
  turnId: "turn-1",
} as const;

let filled: string[] = [];

const helperRunning: TestBody = async (_$, on) => {
  filled = [];
  // The first read finds the helper running; later reads see what the mod wrote.
  let isSeeded = false;
  on("state.get", (_, e, next) => {
    if (isSeeded) return next(e);
    isSeeded = true;
    return { value: { value: { agentId: HELPER_TURN.agentId, tools: 0, last: "" }, version: 1 } };
  });
  on("prompt.read", () => ({ value: { text: "my draft", cursor: 0 } }));
  on("prompt.fill", (_, e) => {
    filled.push(e.text);
    return { isFilled: true };
  });
  on("ui.toast", () => ({ value: undefined }));
  on("turn.complete", (_, e) => ({ text: e.answer }));
};

test("/better spawns the helper with the typed prompt", async ($, on) => {
  const prompts: string[] = [];
  on("agent.spawn", (_, e) => {
    prompts.push(e.prompt);
    return { model: "inherit" };
  });

  await $.command.run({
    command: "better",
    args: "fix the login bug",
    origin: { kind: "composer" },
    presentation: { isFullscreen: false, columns: 80 },
  });
  expect(prompts).toHaveLength(1);
  expect(prompts[0]).toMatch(/fix the login bug/);
});

test("the helper's improved prompt fills the prompt box, above the draft", async ($, on) => {
  await helperRunning($, on);
  await $.turn.complete({
    ...HELPER_TURN,
    answer: "(1) to (4)...\n<improved-prompt>\nFix the login bug in auth.ts\n</improved-prompt>",
  });
  expect(filled).toEqual(["Fix the login bug in auth.ts\n\nmy draft"]);
});

test("a helper reply with no marked prompt leaves the prompt box alone", async ($, on) => {
  await helperRunning($, on);
  await $.turn.complete({ ...HELPER_TURN, answer: "I could not find the code." });
  expect(filled).toEqual([]);
});

test("another agent's turn leaves the prompt box alone", async ($, on) => {
  await helperRunning($, on);
  await $.turn.complete({
    ...HELPER_TURN,
    agentId: "someone-else",
    answer: "<improved-prompt>not ours</improved-prompt>",
  });
  expect(filled).toEqual([]);
});
