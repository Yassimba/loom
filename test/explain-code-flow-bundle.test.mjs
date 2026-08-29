import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";

test("explain-code-flow bundle compiler regressions", () => {
  const result = spawnSync("python3", ["skills/explain-code-flow/scripts/test-bundle.py"], {
    encoding: "utf8",
  });
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
});
