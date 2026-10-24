import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

test("release workflow publishes every released plugin before pinning", async () => {
  const workflow = await readFile(join(repoRoot, ".github/workflows/release.yml"), "utf8");

  assert.match(workflow, /\^plugins\/\.\*--release_created\$/);
  assert.match(workflow, /PACKAGE: \$\{\{ matrix\.path \}\}/);
  assert.match(workflow, /npm publish "\.\/\$PACKAGE"/);
  // A private package (a Claude Code mod) is released by tag but never published to npm.
  assert.match(
    workflow,
    /jq -r '\.private \/\/ false' "\.\/\$PACKAGE\/package\.json"\)" = true \]; then exit 0; fi/,
  );
  assert.doesNotMatch(workflow, /npm publish \.\/plugins\/pi-fast/);
  assert.match(workflow, /needs: \[release-please, binaries, npm\]/);
  assert.match(workflow, /needs\.npm\.result == 'success' \|\| needs\.npm\.result == 'skipped'/);
  // A CLI-only release skips npm; both pin and the published install must still run.
  assert.match(workflow, /!cancelled\(\) && needs\.binaries\.result == 'success'/);
  assert.match(
    workflow,
    /full-install:\n {4}needs: pin\n(?: {4}#.*\n)* {4}if: "!cancelled\(\) && needs\.pin\.result == 'success'"/,
  );
});
