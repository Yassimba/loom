/**
 * Render the Mermaid fixture corpus with two renderer revisions and show the
 * art side by side with the metrics that moved.
 *
 *   node --experimental-strip-types scripts/mermaid-compare.ts [--old <root>] [--new <root>] [fixture...]
 *
 * `--old` defaults to a detached worktree of the baseline commit at
 * /tmp/loom-mermaid-baseline (created on first run); `--new` to this repo.
 * Each revision renders in its own Node process so the two module graphs
 * never mix. Exit status 1 when the new revision has a hard failure the old
 * one did not.
 */

import { execFileSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { type FixtureResult, HARD_KEYS, type Metrics } from "./mermaid-metrics.ts";

const BASELINE = "57814fe3";
const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "..");
const fixtureDir = join(repo, "test/fixtures/mermaid");

function parseArgs(argv: string[]): { oldRoot: string; newRoot: string; fixtures: string[] } {
  let oldRoot = "/tmp/loom-mermaid-baseline";
  let newRoot = repo;
  const fixtures: string[] = [];
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--old") oldRoot = resolve(argv[++i]);
    else if (argv[i] === "--new") newRoot = resolve(argv[++i]);
    else fixtures.push(argv[i]);
  }
  return { oldRoot, newRoot, fixtures };
}

function ensureBaseline(root: string): void {
  if (existsSync(join(root, "plugins/pi-loom-mermaid"))) return;
  execFileSync("git", ["worktree", "add", "--detach", root, BASELINE], {
    cwd: repo,
    stdio: "inherit",
  });
}

function render(root: string, files: string[]): FixtureResult[] {
  const out = execFileSync(
    process.execPath,
    ["--experimental-strip-types", join(here, "mermaid-metrics.ts"), root, ...files],
    { encoding: "utf8", maxBuffer: 1 << 26 },
  );
  return JSON.parse(out) as FixtureResult[];
}

const SHOWN: (keyof Metrics)[] = [
  "width",
  "height",
  "area",
  "crossings",
  "bends",
  "routedLength",
  "marginRight",
  "marginBottom",
  ...HARD_KEYS,
];

function sideBySide(a: string[], b: string[], gap = 4): string {
  const wa = Math.max(0, ...a.map((l) => [...l].length));
  const rows = Math.max(a.length, b.length);
  const lines: string[] = [];
  for (let i = 0; i < rows; i++) {
    const left = a[i] ?? "";
    lines.push(left + " ".repeat(wa - [...left].length + gap) + (b[i] ?? ""));
  }
  return lines.join("\n");
}

function hardFailures(r: FixtureResult): string[] {
  const out: string[] = [];
  if (r.metrics === null) out.push("null render");
  else for (const k of HARD_KEYS) if (r.metrics[k] > 0) out.push(`${k}=${r.metrics[k]}`);
  if (!r.deterministic) out.push("nondeterministic");
  return out;
}

const { oldRoot, newRoot, fixtures } = parseArgs(process.argv.slice(2));
ensureBaseline(oldRoot);
const files =
  fixtures.length > 0
    ? fixtures.map((f) => (existsSync(f) ? f : join(fixtureDir, `${f}.mmd`)))
    : readdirSync(fixtureDir)
        .filter((f) => f.endsWith(".mmd"))
        .sort()
        .map((f) => join(fixtureDir, f));

const olds = render(oldRoot, files);
const news = render(newRoot, files);
let regressed = false;
for (let i = 0; i < files.length; i++) {
  const o = olds[i];
  const n = news[i];
  console.log(`\n=== ${n.name}`);
  console.log(sideBySide(o.plain ?? ["<null>"], n.plain ?? ["<null>"]));
  const table: string[] = [];
  for (const k of SHOWN) {
    const ov = o.metrics?.[k] ?? "-";
    const nv = n.metrics?.[k] ?? "-";
    const mark = ov === nv ? " " : "*";
    table.push(`${mark}${k}=${ov}\u2192${nv}`);
  }
  console.log(table.join("  "));
  const oldBad = hardFailures(o);
  const newBad = hardFailures(n);
  if (oldBad.length > 0) console.log(`old hard failures: ${oldBad.join(", ")}`);
  if (newBad.length > 0) console.log(`NEW hard failures: ${newBad.join(", ")}`);
  if (newBad.some((f) => !oldBad.includes(f))) regressed = true;
}
process.exit(regressed ? 1 : 0);
