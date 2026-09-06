import assert from "node:assert/strict";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";
import {
  HARD_KEYS,
  loadRegistry,
  type Metrics,
  renderFixture,
} from "../scripts/mermaid-metrics.ts";

/**
 * Every fixture must render, render the same twice, and never get worse on a
 * tracked metric than `baseline.json` records. Metrics that improve are fine;
 * commit the new numbers with `UPDATE_MERMAID_BASELINE=1 npm test` so the
 * ratchet only ever tightens.
 */
const dir = join(import.meta.dirname, "fixtures/mermaid");
const baselinePath = join(dir, "baseline.json");
const TRACKED = [...HARD_KEYS, "crossings", "area"] as const;
type Tracked = Pick<Metrics, (typeof TRACKED)[number]>;

const registry = await loadRegistry(join(import.meta.dirname, ".."));
const fixtures = readdirSync(dir)
  .filter((f) => f.endsWith(".mmd"))
  .sort();
const baseline: Record<string, Tracked> = JSON.parse(readFileSync(baselinePath, "utf8"));
const seen: Record<string, Tracked> = {};

for (const file of fixtures) {
  const name = file.slice(0, -4);
  test(`mermaid fixture ${name}`, () => {
    const r = renderFixture(registry, name, readFileSync(join(dir, file), "utf8"));
    const metrics = r.metrics;
    assert.ok(metrics, "render returned null");
    assert.ok(r.deterministic, "repeated render differs");
    const got = Object.fromEntries(TRACKED.map((k) => [k, metrics[k]])) as Tracked;
    seen[name] = got;
    const want = baseline[name];
    if (process.env.UPDATE_MERMAID_BASELINE || want === undefined) return;
    for (const k of TRACKED) assert.ok(got[k] <= want[k], `${k}: ${got[k]} > baseline ${want[k]}`);
  });
}

test.after(() => {
  if (process.env.UPDATE_MERMAID_BASELINE) {
    writeFileSync(baselinePath, `${JSON.stringify(seen, null, 2)}\n`);
  }
});
