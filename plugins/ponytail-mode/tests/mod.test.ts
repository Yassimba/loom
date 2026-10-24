import { expect, type TestBody, test } from "claude-code/testing";

// Capability: ponytail-mode keeps the ponytail section in the system prompt and
// follows level switches. Seam: the engine's prompt.submit and prompt.compose
// events, with the test's hooks in place of the engine beneath the mod.
// Fails when: the section is missing at start, a level switch is ignored, or
// "stop ponytail" leaves the section in the prompt.

type Engine = Parameters<TestBody>[0];

const engine: TestBody = async (_$, on) => {
  on("prompt.compose", () => ({ sections: [{ id: "intro", text: "engine", scope: "shared" }] }));
  on("prompt.submit", (_, e) => ({ text: e.text }));
};

async function typed($: Engine, text: string): Promise<void> {
  const [name, ...rest] = text.startsWith("/") ? text.slice(1).split(" ") : [];
  const command = name ? { name, args: rest.join(" ") } : undefined;
  await $.prompt.submit({ text, wait: false, origin: { kind: "composer" }, command });
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

test("/ponytail ultra switches the level", async ($, on) => {
  await engine($, on);
  await typed($, "/ponytail ultra");
  expect(await ponytailText($)).toMatch(/level: ultra/);
});

test("stop ponytail removes the section, and /ponytail lite brings it back", async ($, on) => {
  await engine($, on);
  await typed($, "ok, stop ponytail please");
  expect(await ponytailText($)).toBeUndefined();
  await typed($, "/ponytail lite");
  expect(await ponytailText($)).toMatch(/level: lite/);
});

test("an unknown argument leaves the level as it was", async ($, on) => {
  await engine($, on);
  await typed($, "/ponytail review");
  expect(await ponytailText($)).toMatch(/level: full/);
});
