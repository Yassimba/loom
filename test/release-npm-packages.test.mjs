import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

test("release workflow publishes every released plugin before pinning", async () => {
  const workflow = await readFile(join(repoRoot, ".github/workflows/release.yml"), "utf8");

  assert.match(workflow, /\^plugins\/\.\*--release_created\$/);
  assert.match(workflow, /npm publish \.\/\$\{\{ matrix\.path \}\}/);
  assert.doesNotMatch(workflow, /npm publish \.\/plugins\/pi-fast/);
  assert.match(workflow, /needs: \[release-please, binaries, npm\]/);
  assert.match(workflow, /needs\.npm\.result == 'success' \|\| needs\.npm\.result == 'skipped'/);
  // A CLI-only release skips npm; both pin and the published install must still run.
  assert.match(workflow, /!cancelled\(\) && needs\.binaries\.result == 'success'/);
  assert.match(
    workflow,
    /full-install:\n    needs: pin\n(?:    #.*\n)*    if: "!cancelled\(\) && needs\.pin\.result == 'success'"/,
  );
});
