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

test("back-edge side exit clears a wider source in the dataset lifecycle", () => {
  const proposed = `H[Dataset finishes] --> I[Lifecycle module retains result]
    I --> J{Persistence acknowledged?}
    J -->|Yes| K[Publish progress]
    K --> L{Every accepted Dataset durable?}
    L -->|No| I
    L -->|Yes| M[Publish complete terminal Run Result]
    J -->|No| N[Keep failure and pending results]
    N --> O[Recovery policy needs a decision]`;
  const before = `subgraph Before
    A[Dataset finishes] --> B[Append live result]
    B --> C{Dataset write succeeds?}
    C -->|Yes| D[Publish progress]
    C -->|No| E[Stop collecting results]
    E --> F[Mark Run Record terminal]
    F --> G[History can lack live results]
    end`;
  const failure = "J -->|No| N[Keep failure and pending results]";
  const right = proposed
    .replace(failure, "")
    .replace("J -->|Yes| K", `${failure}\n    J -->|Yes| K`);
  for (const source of [
    `flowchart TB\n${proposed}`,
    `flowchart TB\n${before}\nsubgraph Proposed\n${proposed}\nend`,
    `flowchart TB\n${right}`,
    `flowchart TB\n${right}\nL --> L`,
  ]) {
    const r = renderFixture(registry, "dataset-lifecycle", source);
    assert.ok(r.plain && r.metrics);
    assert.ok(r.deterministic);
    const text = r.plain.join("\n");
    if (source.endsWith("L --> L")) {
      const row = r.plain.findIndex((line) => line.includes("Every accepted Dataset"));
      assert.match(r.plain[row - 1], /╧/, "a self-loop owning the side keeps the top fallback");
    } else {
      assert.match(
        text,
        /└─+║ +durable\?|durable\? +║─+┘/,
        "No leaves the source side directly into its return lane",
      );
    }
    for (const key of HARD_KEYS) assert.equal(r.metrics[key], 0, key);
  }
});

test.after(() => {
  if (process.env.UPDATE_MERMAID_BASELINE) {
    writeFileSync(baselinePath, `${JSON.stringify(seen, null, 2)}\n`);
  }
});
