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
    // Every edge label the source writes shows up, at least by its head:
    // a label that does not fit is truncated with an ellipsis, never dropped.
    const src = readFileSync(join(dir, file), "utf8");
    const plain = (r.plain ?? []).join("\n");
    for (const m of src.matchAll(/-->\s*\|([^|]+)\|/g)) {
      const head = m[1].trim().slice(0, 6);
      assert.ok(plain.includes(head), `edge label ${JSON.stringify(m[1].trim())} was dropped`);
    }
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
      // The self-loop owns one side, so the return climbs the other: both
      // stay on the box's sides and neither needs the top.
      assert.match(r.plain[row], /└─+╢/, "the return leaves the side away from the loop");
      assert.match(r.plain[row], /╟─╮|╭─╢/, "the self-loop keeps its own side");
    } else {
      assert.match(
        text,
        /└─+╢ (Every accepted|.*durable\?)|durable\? +╟─+┘/,
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

/**
 * Seeded random diagrams of every graph-drawn kind: none may throw, differ
 * between two renders, or trip a hard metric (a broken link, a line under
 * text, a dangling head, a line behind a box).
 */
test("random diagrams hold the hard invariants", () => {
  let seed = 20240501;
  const rnd = (n: number): number => {
    seed = (Math.imul(seed, 1103515245) + 12345) & 0x7fffffff;
    return (seed >> 8) % n;
  };
  const pick = <T>(xs: T[]): T => xs[rnd(xs.length)];
  const id = (): string => `N${rnd(8)}`;
  const label = (): string => pick(["ok", "retry", "fails", "timeout", "Order Service", "配送"]);
  const lines = (header: string, line: () => string): string =>
    [header, ...Array.from({ length: 3 + rnd(10) }, line)].join("\n");
  const kinds: Record<string, () => string> = {
    flowchart: () =>
      lines(
        `flowchart ${pick(["TD", "LR"])}`,
        () => `  ${id()} -->${rnd(2) ? `|${label()}|` : ""} ${id()}`,
      ),
    // Groups: nodes declared inside one or two frames (sometimes nested,
    // sometimes with their own direction), edges between nodes and frames.
    subgraphs: () => {
      const out = [`flowchart ${pick(["TD", "LR"])}`];
      const groups = 1 + rnd(3);
      // Group g owns N(2g) and N(2g+1); the rest stand outside.
      for (let g = 0; g < groups; g++) {
        out.push(`  subgraph G${g}[${label()}]`);
        if (!rnd(3)) out.push(`    direction ${pick(["TB", "LR"])}`);
        out.push(`    N${2 * g}`);
        out.push(
          ...(rnd(3)
            ? [`    N${2 * g + 1}`]
            : [`    subgraph H${g}`, `      N${2 * g + 1}`, "    end"]),
        );
        out.push("  end");
      }
      const owner = (end: string): string =>
        end[0] === "G" ? end : `G${Number(end.slice(1)) >> 1}`;
      for (let k = 2 + rnd(8); k > 0; k--) {
        const [a, b] = [rnd(4) ? id() : `G${rnd(groups)}`, rnd(4) ? id() : `G${rnd(groups)}`];
        // A frame linked to itself or to its own member is not a drawing.
        if ((a[0] === "G" || b[0] === "G") && owner(a) === owner(b)) continue;
        out.push(
          `  ${a} ${pick(["-->", "-.->", "==>", "<-->", "--o", "--x"])}${rnd(2) ? `|${label()}|` : ""} ${b}`,
        );
      }
      return out.join("\n");
    },
    composite: () => {
      const out = ["stateDiagram-v2"];
      for (let g = 0; g < 1 + rnd(2); g++) {
        out.push(`  state C${g} {`);
        for (let k = 1 + rnd(3); k > 0; k--)
          out.push(`    ${pick(["[*]", id()])} --> ${pick(["[*]", id()])}`);
        out.push("  }");
      }
      for (let k = 2 + rnd(6); k > 0; k--) {
        out.push(
          `  ${pick(["[*]", id(), "C0"])} --> ${pick([id(), "C0", "[*]"])}${rnd(2) ? `: ${label()}` : ""}`,
        );
      }
      return out.join("\n");
    },
    state: () =>
      lines(
        "stateDiagram-v2",
        () => `  ${pick(["[*]", id(), id()])} --> ${id()}${rnd(2) ? `: ${label()}` : ""}`,
      ),
    class: () =>
      lines(
        `classDiagram${rnd(3) ? "" : "\n  direction LR"}`,
        () =>
          `  ${id()} ${pick(["<|--", "*--", "o--", "-->", "..>"])} ${id()}${rnd(2) ? ` : ${label()}` : ""}`,
      ),
    er: () =>
      lines(
        "erDiagram",
        () =>
          `  ${id()} ${pick(["||", "}o", "}|"])}--${pick(["||", "o{", "|{"])} ${id()} : ${label().replace(" ", "_")}`,
      ),
  };
  for (const [kind, generate] of Object.entries(kinds)) {
    for (let i = 0; i < 60; i++) {
      const source = generate();
      const r = renderFixture(registry, `${kind}-${i}`, source);
      assert.ok(r.metrics, `${kind} drew nothing:\n${source}`);
      assert.ok(r.deterministic, `${kind} differs between renders:\n${source}`);
      for (const key of HARD_KEYS) {
        assert.equal(
          r.metrics[key],
          0,
          `${key} in ${kind}:\n${source}\n${(r.plain ?? []).join("\n")}`,
        );
      }
    }
  }
});
