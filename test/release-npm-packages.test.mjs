import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

test("release workflow publishes every released plugin package", async () => {
  const workflow = await readFile(join(repoRoot, ".github/workflows/release.yml"), "utf8");

  assert.match(workflow, /\^plugins\/\.\*--release_created\$/);
  assert.match(workflow, /npm publish \.\/\$\{\{ matrix\.path \}\}/);
  assert.doesNotMatch(workflow, /npm publish \.\/plugins\/openai-fast/);
});
