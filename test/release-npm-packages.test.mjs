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

// Capability: a release keeps the root lockfile in step with each plugin.
// Seam: release-please-config.json and package-lock.json (external contract).
// Fails when: a plugin has no lockfile extra-file, or the lockfile holds a
// version its package.json no longer has (the drift every release used to leave).
test("every released plugin keeps its root lockfile version in step", async () => {
  const config = JSON.parse(await readFile(join(repoRoot, "release-please-config.json"), "utf8"));
  const lock = JSON.parse(await readFile(join(repoRoot, "package-lock.json"), "utf8"));
  const plugins = Object.keys(config.packages).filter((path) => path.startsWith("plugins/"));
  assert.ok(plugins.length > 0);
  for (const path of plugins) {
    assert.ok(
      config.packages[path]["extra-files"]?.some(
        (file) =>
          file.path === "/package-lock.json" && file.jsonpath === `$.packages['${path}'].version`,
      ),
      `${path} has no lockfile extra-file`,
    );
    const manifest = JSON.parse(await readFile(join(repoRoot, path, "package.json"), "utf8"));
    assert.equal(lock.packages[path]?.version, manifest.version, `${path} lockfile version`);
  }
});
