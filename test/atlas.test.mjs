import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";

for (const [name, script] of [
  ["atlas retrieval and refresh", "skills/system-atlas/scripts/test_atlas.py"],
  ["compact and legacy Blueprint", "skills/blueprint/scripts/test_check_blueprint.py"],
]) {
  test(name, () => {
    const result = spawnSync("python3", [script], { encoding: "utf8", timeout: 60_000 });
    assert.equal(result.status, 0, `${result.error ?? ""}\n${result.stdout}\n${result.stderr}`);
  });
}
