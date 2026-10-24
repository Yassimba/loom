import { expect, type TestBody, test } from "claude-code/testing";

// Capability: ponytail-mode turns ponytail on for every session and follows
// level switches. Seam: the engine's prompt.submit and prompt.compose events,
// with the test's hooks in place of the engine beneath the mod.
// Fails when: the section is missing at start, a switch does not reach the
// model beside its own prompt (the engine composes the system prompt once per
// session), or a recompose (compaction, resume) forgets the switch.

type Engine = Parameters<TestBody>[0];

let delivered: readonly string[] | undefined;

const engine: TestBody = async (_$, on) => {
  delivered = undefined;
  on("prompt.compose", () => ({ sections: [{ id: "intro", text: "engine", scope: "shared" }] }));
  on("prompt.submit", (_, e) => {
    delivered = e.context;
    return { text: e.text };
  });
};

async function typed($: Engine, text: string): Promise<string> {
  const [name, ...rest] = text.startsWith("/") ? text.slice(1).split(" ") : [];
  const command = name ? { name, args: rest.join(" ") } : undefined;
  await $.prompt.submit({ text, wait: false, origin: { kind: "composer" }, command });
  return (delivered ?? []).join("\n");
}

async function ponytailText($: Engine): Promise<string | undefined> {
  const { sections } = await $.prompt.compose({
    model: "claude-opus-5-5",
    promptModel: "claude-opus-5-5",
    surfaces: ["terminal"],
    tools: ["Skill"],
    outputStyle: null,
    traits: ["skills"],
  });
  return sections.find((s) => s.id === "ponytail-mode:rules")?.text;
}

test("a new session carries the section at level full", async ($, on) => {
  await engine($, on);
  expect(await ponytailText($)).toMatch(/level: full/);
});

test("/ponytail ultra tells the model beside the prompt and survives a recompose", async ($, on) => {
  await engine($, on);
  expect(await typed($, "/ponytail ultra")).toMatch(/level: ultra/);
  expect(await ponytailText($)).toMatch(/level: ultra/);
});

test("stop ponytail tells the model it is off, and /ponytail lite brings it back", async ($, on) => {
  await engine($, on);
  expect(await typed($, "ok, stop ponytail please")).toMatch(/now off/);
  expect(await ponytailText($)).toBeUndefined();
  expect(await typed($, "/ponytail lite")).toMatch(/level: lite/);
  expect(await ponytailText($)).toMatch(/level: lite/);
});

test("other prompts and unknown arguments add nothing and keep the level", async ($, on) => {
  await engine($, on);
  expect(await typed($, "fix the login bug")).toBe("");
  expect(await typed($, "/ponytail review")).toBe("");
  expect(await ponytailText($)).toMatch(/level: full/);
});
