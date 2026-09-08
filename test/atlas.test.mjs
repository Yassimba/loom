import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";

test("atlas retrieval and refresh", () => {
  const result = spawnSync("python3", ["skills/system-atlas/scripts/test_atlas.py"], {
    encoding: "utf8",
    timeout: 60_000,
  });
  assert.equal(result.status, 0, `${result.error ?? ""}\n${result.stdout}\n${result.stderr}`);
});
