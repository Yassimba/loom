import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { parse } from "yaml";

test("Windows smoke-test skills exist in the catalog and still cover Beads", async () => {
  const workflow = parse(
    await readFile(new URL("../.github/workflows/full-install.yml", import.meta.url), "utf8"),
  );
  const script = await readFile(
    new URL("../scripts/e2e-install-windows.ps1", import.meta.url),
    "utf8",
  );
  const catalog = JSON.parse(
    await readFile(new URL("../cli/loom/setup-catalog.json", import.meta.url), "utf8"),
  );
  const defaultSkill = script.match(/\$Skill = .*else \{ "([^"]+)" \}/)?.[1];
  const beadsSkill = script.match(/\$ExpectBeads = \$Skill -eq "([^"]+)"/)?.[1];
  assert.ok(defaultSkill, "PowerShell must define a default smoke-test skill");
  assert.ok(beadsSkill, "PowerShell must identify the skill that exercises Beads");
  const matrix = workflow.jobs.windows.strategy.matrix.include;
  for (const skill of [defaultSkill, ...matrix.map((row) => row.skill)]) {
    assert.ok(
      catalog.resources.some((resource) => resource.id === `skill:${skill}`),
      `unknown smoke-test skill: ${skill}`,
    );
  }
  assert.ok(
    matrix.some((row) => row.skill === beadsSkill),
    "the matrix must exercise Beads",
  );
  const resource = catalog.resources.find((resource) => resource.id === `skill:${beadsSkill}`);
  for (const tool of [
    "github:Dicklesworthstone/beads_rust",
    "github:Dicklesworthstone/beads_viewer",
  ]) {
    assert.ok(resource?.dependencies.includes(tool), `${beadsSkill} must install ${tool}`);
  }
});
