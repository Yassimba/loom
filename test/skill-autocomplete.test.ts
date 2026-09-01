import assert from "node:assert/strict";
import test from "node:test";
import { extractSkillQuery, skillItems } from "../plugins/skill-autocomplete/index.ts";

test("extracts a $ skill query anywhere before the cursor", () => {
  assert.equal(extractSkillQuery("use $brain"), "brain");
  assert.equal(extractSkillQuery("$"), "");
  assert.equal(extractSkillQuery("use $brain later"), undefined);
});

test("lists only skills as $ mentions", () => {
  const items = skillItems(
    [
      {
        name: "skill:brainstorming",
        description: "Explore an idea",
        source: "skill",
        sourceInfo: {} as never,
      },
      {
        name: "reload",
        description: "Reload Pi",
        source: "extension",
        sourceInfo: {} as never,
      },
    ],
    "brain",
  );

  assert.deepEqual(items, [
    { value: "$brainstorming", label: "$brainstorming", description: "Explore an idea" },
  ]);
});
