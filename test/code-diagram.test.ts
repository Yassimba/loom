import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { setTimeout as delay } from "node:timers/promises";
import { pathToFileURL } from "node:url";
import React from "react";

import { defineActors, defineAnchors } from "../skills/code-diagram/scripts/src/authoring/core.ts";
import {
  defineSoftwareActors,
  defineSoftwareStores,
} from "../skills/code-diagram/scripts/src/authoring/session.ts";
import { calls } from "../skills/code-diagram/scripts/src/diagrams/call-stack-diff/authoring.ts";
import {
  callStackBrowserSchema,
  diffCallStacks,
} from "../skills/code-diagram/scripts/src/diagrams/call-stack-diff/model.ts";
import { defineStores } from "../skills/code-diagram/scripts/src/diagrams/database-lens/authoring.ts";
import {
  type CapturedDatabaseLens,
  compileDatabaseLens,
  createDatabaseLensComponents,
} from "../skills/code-diagram/scripts/src/diagrams/database-lens/model.ts";
import { defineSoftwareMap } from "../skills/code-diagram/scripts/src/diagrams/software-map/model.ts";
import {
  collapseInlineC4Node,
  projectInlineC4,
} from "../skills/code-diagram/scripts/src/diagrams/software-map/projection.ts";
import { patchChangedLines } from "../skills/code-diagram/scripts/src/document/diff.ts";
import { createSurfaceRegistry } from "../skills/code-diagram/scripts/src/document/registry.ts";

const root = process.cwd();
const cli = path.join(root, "skills/code-diagram/scripts/code-diagram.ts");
const fixture = path.join(root, "skills/code-diagram/fixtures/review.mdx");

function run(args: string[]) {
  return spawnSync(process.execPath, ["--import", "tsx", cli, ...args], {
    cwd: root,
    encoding: "utf8",
  });
}

async function waitFor(check: () => boolean | Promise<boolean>, message: string) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (await check()) return;
    await delay(50);
  }
  assert.fail(message);
}

function writeReview(dir: string, data: string, mdx: string): string {
  const review = path.join(dir, "review.mdx");
  writeFileSync(path.join(dir, "data.ts"), data);
  writeFileSync(review, mdx);
  return review;
}

function git(dir: string, args: string[]) {
  const result = spawnSync("git", ["-C", dir, ...args], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
}

function commitFixture(dir: string) {
  git(dir, ["init", "-q"]);
  git(dir, ["config", "user.email", "code-diagram@example.test"]);
  git(dir, ["config", "user.name", "Code Diagram Test"]);
  git(dir, ["add", "."]);
  git(dir, ["-c", "commit.gpgsign=false", "commit", "-qm", "fixture"]);
}

test("code-diagram builds Review's review.mdx + data.ts surface into one offline HTML file", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-"));
  const output = path.join(dir, "sequence.html");
  const checked = run(["check", fixture, "--repo", root]);
  assert.equal(checked.status, 0, checked.stderr);
  const built = run(["build", fixture, "--repo", root, "--out", output]);
  assert.equal(built.status, 0, built.stderr);
  const html = readFileSync(output, "utf8");
  assert.match(html, /^<!doctype html>/);
  assert.match(html, /Content-Security-Policy/);
  assert.match(html, /Submit and process/);
  assert.doesNotMatch(html, /A caller submits an input/);
  assert.match(html, /const request = normalize\(input\)/);
  assert.match(html, /data-code-diagram-index="0"/);
  const shell = html.match(/<body>(.*?)<script>/s)?.[1] ?? "";
  assert.doesNotMatch(shell, /<article|code-diagram-revision|<h1|<p/i);
  assert.equal(shell.match(/data-code-diagram-kind=/g)?.length, 1);
  assert.doesNotMatch(html, /<script\s+[^>]*src=/i);
  assert.doesNotMatch(html, /<link\s+[^>]*href=/i);
  assert.doesNotMatch(html, /__CODE_DIAGRAM_LIBAVOID_WASM_URL__/);
  assert.equal(html.includes(root), false, "generated HTML leaked the repository path");
});

test("each registered surface builds alone with only its required browser assets", () => {
  const cases = [
    {
      kind: "sequence",
      data: `import { defineActors, defineAnchors } from "virtual:progressive-review-authoring";
export const actors = defineActors({ a: { label: "A" }, b: { label: "B" } });
export const anchors = defineAnchors({ call: { title: "Call", peek: { file: "skills/code-diagram/fixtures/source.ts", fromLine: 1, toLine: 1 } } });
export const messages = [{ from: actors.a, to: actors.b, label: "call", anchor: anchors.call }];`,
      mdx: `import { messages } from "./data";\n\n# Sequence only\n\n<SequenceDiagram label="Only" messages={messages} />`,
      libavoid: false,
    },
    {
      kind: "call-stack-diff",
      data: `import { defineAnchors } from "virtual:progressive-review-authoring";
export const anchors = defineAnchors({ a: { title: "A", peek: { file: "skills/code-diagram/fixtures/source.ts", fromLine: 1, toLine: 1 } }, b: { title: "B", peek: { file: "skills/code-diagram/fixtures/source.ts", fromLine: 1, toLine: 1 } } });
export const base = [anchors.a]; export const head = [anchors.a];`,
      mdx: `import { base, head } from "./data";\n\n# Stack only\n\n<CallStackDiff base={base} head={head} />`,
      libavoid: false,
    },
    {
      kind: "database-lens",
      data: `import { defineActors, defineAnchors, defineStores } from "virtual:progressive-review-authoring";
export const actors = defineActors({ app: { label: "App" } });
export const anchors = defineAnchors({ read: { title: "Read", peek: { file: "skills/code-diagram/fixtures/source.ts", fromLine: 1, toLine: 1 } } });
export const stores = defineStores({ db: { kind: "relational", label: "DB", tables: { users: { schema: { id: { type: "number" } } } } } });`,
      mdx: `import { actors, anchors, stores } from "./data";\n\n# Database only\n\n<DatabaseLens stores={stores}><DbUseCase id="read" label="Read"><DbRead from={stores.db.tables.users.id} to={actors.app} label="read" anchor={anchors.read} /></DbUseCase></DatabaseLens>`,
      libavoid: true,
    },
    {
      kind: "software-map",
      data: "export {};",
      mdx: "# Software map only",
      artifact: `import { defineSoftwareMap } from "@dev.fast/progressive-review/software-map-model";\nexport default defineSoftwareMap({ systems: { app: { label: "App" } } });`,
      libavoid: true,
    },
  ] as const;

  for (const surface of cases) {
    const dir = mkdtempSync(path.join(tmpdir(), `code-diagram-${surface.kind}-`));
    const review = writeReview(dir, surface.data, surface.mdx);
    if ("artifact" in surface) writeFileSync(path.join(dir, "software-map.ts"), surface.artifact);
    const output = path.join(dir, "out.html");
    const built = run(["build", review, "--repo", root, "--out", output]);
    assert.equal(built.status, 0, `${surface.kind}: ${built.stderr}`);
    const html = readFileSync(output, "utf8");
    const shell = html.match(/<body>(.*?)<script>/s)?.[1] ?? "";
    assert.equal(shell.match(/data-code-diagram-kind=/g)?.length, 1, surface.kind);
    assert.match(shell, new RegExp(`data-code-diagram-kind="${surface.kind}"`));
    assert.equal(
      html.includes("__CODE_DIAGRAM_LIBAVOID_WASM_URL__"),
      surface.libavoid,
      `${surface.kind} assets`,
    );
  }
});

test("code-diagram rejects unsupported Review components and stale anchors", () => {
  const unsupportedDir = mkdtempSync(path.join(tmpdir(), "code-diagram-unsupported-"));
  const unsupported = writeReview(
    unsupportedDir,
    "export {};\n",
    '# No\n\n<StateDiagram label="No" />\n',
  );
  const unsupportedResult = run(["check", unsupported, "--repo", unsupportedDir]);
  assert.notEqual(unsupportedResult.status, 0);
  assert.match(unsupportedResult.stderr, /StateDiagram/);

  const staleDir = mkdtempSync(path.join(tmpdir(), "code-diagram-stale-"));
  mkdirSync(path.join(staleDir, "src"));
  writeFileSync(path.join(staleDir, "src", "source.ts"), "export const value = 1;\n");
  const stale = writeReview(
    staleDir,
    `import { defineActors, defineAnchors } from "virtual:progressive-review-authoring";
export const actors = defineActors({ a: { label: "A" }, b: { label: "B" } });
export const anchors = defineAnchors({ stale: { title: "Stale", peek: { file: "src/source.ts", fromLine: 999, toLine: 999 } } });
export const messages = [{ from: actors.a, to: actors.b, label: "missing", anchor: anchors.stale }];
`,
    `import { messages } from "./data.ts";

# Stale

<SequenceDiagram label="Stale" messages={messages} />
`,
  );
  const staleResult = run(["check", stale, "--repo", staleDir]);
  assert.notEqual(staleResult.status, 0);
  assert.match(staleResult.stderr, /EVIDENCE_RANGE_INVALID/);
});

test("data.ts is type-checked against Review's typed authoring helpers", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-types-"));
  const review = writeReview(
    dir,
    `import { defineActors } from "virtual:progressive-review-authoring";
export const actors = defineActors({ broken: { label: 42 } });
`,
    "# Typed\n",
  );
  const result = run(["check", review, "--repo", dir]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /TYPESCRIPT_INVALID/);
  assert.match(result.stderr, /number.*string|not assignable to type 'string'/s);
});

test("registry and Review model ports cover all four surfaces", () => {
  const modelSource = path.join(
    root,
    "skills/code-diagram/scripts/src/diagrams/software-map/model.ts",
  );
  const registry = createSurfaceRegistry(modelSource);
  assert.deepEqual(
    registry.map((surface) => surface.kind),
    ["sequence", "call-stack-diff", "database-lens", "software-map"],
  );
  const anchors = defineAnchors({
    parent: { title: "Parent", peek: { file: "parent.ts", fromLine: 1, toLine: 1 } },
    child: { title: "Child", peek: { file: "child.ts", fromLine: 1, toLine: 1 } },
  });
  const assertion = calls(anchors.parent, anchors.child, "queue");
  assert.equal(assertion.parent, anchors.parent);
  assert.equal(assertion.child, anchors.child);
  assert.deepEqual(
    diffCallStacks([anchors.parent], [anchors.child]).map((row) => row.change),
    ["removed", "added"],
  );
  assert.throws(() =>
    callStackBrowserSchema.parse({
      rows: [
        {
          entry: { bogus: true },
          change: "added",
          depth: 0,
          source: { file: "source.ts", fromLine: 1, toLine: 1, lines: [] },
        },
      ],
    }),
  );
  const lines = patchChangedLines("@@ -3,1 +3,1 @@\n-old\n+new");
  assert.deepEqual([...lines.deleted], [3]);
  assert.deepEqual([...lines.added], [3]);
  const map = defineSoftwareMap({
    systems: { loom: { containers: { cli: { components: { planner: {} } } } } },
  });
  assert.ok(map.elementsByPath.has("loom.cli.planner"));
  const mapped = defineSoftwareMap({
    systems: {
      app: {
        dataStores: {
          db: {
            kind: "database",
            tables: { users: { schema: { id: { type: "text", pk: true } } } },
          },
        },
      },
    },
  });
  const mappedActors = defineSoftwareActors(mapped, { app: "app" });
  assert.equal(mappedActors.app.softwareMapPath, "app");
  const mappedStores = defineSoftwareStores(mapped, { db: { path: "app.db" } });
  assert.equal(mappedStores.db.tables.users.id.__kind, undefined);
  assert.equal(mappedStores.db.softwareMapPath, "app.db");
  const projection = projectInlineC4({
    model: map,
    expandedNodeIds: new Set(["loom", "loom.cli"]),
  });
  assert.deepEqual(
    projection.nodes.map((node) => node.id),
    ["loom", "loom.cli", "loom.cli.planner"],
  );
  assert.deepEqual([...collapseInlineC4Node(new Set(["loom", "loom.cli"]), "loom")], []);
});

test("complex Loom walkthrough builds every surface from MDX plus a separate map artifact", () => {
  const example = path.join(root, "skills/code-diagram/examples/loom-installer/review.mdx");
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-loom-"));
  const output = path.join(dir, "loom.html");
  const checked = run(["check", example, "--repo", root]);
  assert.equal(checked.status, 0, checked.stderr);
  assert.match(checked.stderr, /1 sequence/);
  assert.match(checked.stderr, /1 call-stack-diff/);
  assert.match(checked.stderr, /1 database-lens/);
  assert.match(checked.stderr, /1 software-map/);
  const built = run(["build", example, "--repo", root, "--out", output]);
  assert.equal(built.status, 0, built.stderr);
  const html = readFileSync(output, "utf8");
  for (const kind of ["sequence", "call-stack-diff", "database-lens", "software-map"])
    assert.match(html, new RegExp(`data-code-diagram-kind="${kind}"`));
  assert.match(html, /Loom installer/);
  assert.match(html, /Persisted local files/);
  assert.match(html, /__CODE_DIAGRAM_LIBAVOID_WASM_URL__/);
});

test("base evidence reads a deleted file from the invocation-pinned HEAD", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-base-"));
  mkdirSync(path.join(dir, "src"));
  writeFileSync(path.join(dir, "src", "old.ts"), "export function oldCall() {}\n");
  const review = writeReview(
    dir,
    `import { defineActors, defineAnchors } from "virtual:progressive-review-authoring";
export const actors = defineActors({ a: { label: "A" }, b: { label: "B" } });
export const anchors = defineAnchors({ old: { title: "Old", peek: { file: "src/old.ts", fromLine: 1, toLine: 1, graph: "base" } } });
export const messages = [{ from: actors.a, to: actors.b, label: "old call", anchor: anchors.old }];
`,
    `import { messages } from "./data.ts";\n\n# Deleted base evidence\n\n<SequenceDiagram label="Base" messages={messages} />\n`,
  );
  commitFixture(dir);
  rmSync(path.join(dir, "src", "old.ts"));
  const checked = run(["check", review, "--repo", dir]);
  assert.equal(checked.status, 0, checked.stderr);
});

test("CallStackDiff rejects a removal marker over unchanged code", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-stack-evidence-"));
  mkdirSync(path.join(dir, "src"));
  writeFileSync(path.join(dir, "src", "same.ts"), "export function same() {}\n");
  const review = writeReview(
    dir,
    `import { defineAnchors } from "virtual:progressive-review-authoring";
export const anchors = defineAnchors({ same: { title: "Same", peek: { file: "src/same.ts", fromLine: 1, toLine: 1, graph: "base" } } });
export const base = [anchors.same]; export const head = [];
`,
    `import { base, head } from "./data.ts";\n\n# Dishonest stack\n\n<CallStackDiff base={base} head={head} />\n`,
  );
  commitFixture(dir);
  const checked = run(["check", review, "--repo", dir]);
  assert.notEqual(checked.status, 0);
  assert.match(checked.stderr, /CALL_STACK_EVIDENCE_INVALID/);
});

test("SoftwareMap accepts only the Diagram Design relationship semantics", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-map-semantics-"));
  const review = writeReview(dir, "export {};\n", "# Relationship semantics\n");
  const semanticKinds = [
    "dependency",
    "http",
    "async",
    "return",
    "optional",
    "primary",
    "forbidden",
    "published",
    "foreign-key",
  ];
  const systems = Object.fromEntries(["source", ...semanticKinds].map((id) => [id, { label: id }]));
  const mapSource = (
    semanticKind: string,
  ) => `import { defineSoftwareMap } from "@dev.fast/progressive-review/software-map-model";
export default defineSoftwareMap(${JSON.stringify({
    systems,
    relationships:
      semanticKind === "all"
        ? semanticKinds.map((kind) => ({
            kind: "semantic",
            semanticKind: kind,
            from: "source",
            to: kind,
          }))
        : [{ kind: "semantic", semanticKind, from: "source", to: "dependency" }],
  })});\n`;
  writeFileSync(path.join(dir, "software-map.ts"), mapSource("all"));
  const valid = run(["check", review, "--repo", dir]);
  assert.equal(valid.status, 0, valid.stderr);
  writeFileSync(path.join(dir, "software-map.ts"), mapSource("magic"));
  const invalid = run(["check", review, "--repo", dir]);
  assert.notEqual(invalid.status, 0);
  assert.match(invalid.stderr, /magic|semanticKind/i);
});

test("invalid adjacent SoftwareMap artifacts fail check", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-map-invalid-"));
  const review = writeReview(dir, "export {};\n", "# Invalid map\n");
  writeFileSync(
    path.join(dir, "software-map.ts"),
    `import { defineSoftwareMap } from "@dev.fast/progressive-review/software-map-model";
export default defineSoftwareMap({ systems: { app: {} }, relationships: [{ kind: "semantic", from: "app", to: "missing" }] });\n`,
  );
  const checked = run(["check", review, "--repo", dir]);
  assert.notEqual(checked.status, 0);
  assert.match(checked.stderr, /SOFTWARE_MAP_INVALID|Invalid software model/);
});

test("code-diagram rejects executable authored HTML", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-script-"));
  const review = writeReview(
    dir,
    "export {};\n",
    "# Unsafe\n\n<script>globalThis.pwned = true</script>\n",
  );
  const result = run(["check", review, "--repo", dir]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /DOCUMENT_HTML_UNSAFE/);
});

test("check rejects compiled values that cannot round-trip through JSON", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-json-safe-"));
  writeFileSync(
    path.join(dir, "review.mdx"),
    `import { actors, anchors, stores } from "./data";

# JSON safety

<DatabaseLens title="Data" stores={stores}>
  <DbUseCase id="read" label="Read">
    <DbRead from={stores.db.tables.users.id} to={actors.app} label="read" anchor={anchors.read} />
  </DbUseCase>
</DatabaseLens>
`,
  );
  const dataPath = path.join(dir, "data.ts");
  const data = (
    example: string,
  ) => `import { defineActors, defineAnchors, defineStores } from "virtual:progressive-review-authoring";
export const actors = defineActors({ app: { label: "App" } });
export const anchors = defineAnchors({ read: { title: "Read", peek: { file: "skills/code-diagram/fixtures/source.ts", fromLine: 1, toLine: 1 } } });
export const stores = defineStores({ db: { kind: "relational", label: "DB", tables: { users: { label: "Users", schema: { id: { type: "number", example: ${example} } } } } } });\n`;
  writeFileSync(dataPath, data("Number.NaN"));
  let result = run(["check", path.join(dir, "review.mdx"), "--repo", root]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /received NaN/);
  writeFileSync(dataPath, data("1n"));
  result = run(["check", path.join(dir, "review.mdx"), "--repo", root]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /bigint/i);
});

test("software-map paths must resolve against the adjacent artifact", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-map-reference-"));
  const review = writeReview(
    dir,
    `import { defineActors, defineAnchors } from "virtual:progressive-review-authoring";
export const actors = defineActors({ missing: { label: "Missing", softwareMapPath: "app.missing" }, ok: { label: "OK" } });
export const anchors = defineAnchors({ hop: { title: "Hop", peek: { file: "source.ts", fromLine: 1, toLine: 1 } } });
export const messages = [{ from: actors.missing, to: actors.ok, label: "hop", anchor: anchors.hop }];
`,
    `import { messages } from "./data.ts";\n\n# Map reference\n\n<SequenceDiagram label="Map" messages={messages} />\n`,
  );
  writeFileSync(path.join(dir, "source.ts"), "export const source = true;\n");
  writeFileSync(
    path.join(dir, "software-map.ts"),
    `import { defineSoftwareMap } from "@dev.fast/progressive-review/software-map-model";
export default defineSoftwareMap({ systems: { app: {} } });\n`,
  );
  const result = run(["check", review, "--repo", dir]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /SOFTWARE_MAP_REFERENCE_INVALID.*app\.missing/s);
});

test("SoftwareMap reads removed element evidence from the pinned base", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-map-base-"));
  mkdirSync(path.join(dir, "src"));
  writeFileSync(path.join(dir, "src", "removed.ts"), "export const removed = true;\n");
  const review = writeReview(dir, "export {};\n", "# Removed map evidence\n");
  writeFileSync(
    path.join(dir, "software-map.ts"),
    `import { defineSoftwareMap } from "@dev.fast/progressive-review/software-map-model";
export default defineSoftwareMap({ systems: { app: { containers: { api: { components: { old: { codeElements: { removed: { changeStatus: "removed", sourceRanges: [{ file: "src/removed.ts", fromLine: 1, toLine: 1 }] } } } } } } } } });\n`,
  );
  commitFixture(dir);
  rmSync(path.join(dir, "src", "removed.ts"));
  const result = run(["check", review, "--repo", dir]);
  assert.equal(result.status, 0, result.stderr);
});

test("DatabaseLens browser model preserves exact schemas and source evidence", async () => {
  const stores = defineStores({
    mixed: {
      kind: "relational",
      label: "Mixed",
      tables: {
        users: {
          schema: {
            id: { type: "text", pk: true },
            profile: {
              type: "object",
              example: { name: "Ada" },
              schema: { name: { type: "text" } },
            },
          },
        },
      },
      documents: {
        audit: { schema: { actor_id: { type: "text", fk: "users.id" } } },
      },
    },
  });
  const actors = defineActors({ api: { label: "API" } });
  const anchors = defineAnchors({
    read: { title: "Read", peek: { file: "source.ts", fromLine: 1, toLine: 1 } },
  });
  let captured: CapturedDatabaseLens | undefined;
  const components = createDatabaseLensComponents((model) => {
    captured = model;
    return React.createElement("div");
  });
  components.DatabaseLens({
    stores,
    children: React.createElement(
      components.DbUseCase,
      { id: "read", label: "Read" },
      React.createElement(components.DbRead, {
        from: stores.mixed.tables.users.profile.name,
        to: actors.api,
        label: "read",
        anchor: anchors.read,
      }),
    ),
  });
  assert.ok(captured);
  assert.deepEqual(
    captured.stores[0].collections.map((collection) => collection.kind),
    ["tables", "documents"],
  );
  assert.deepEqual(captured.stores[0].collections[0].schema.profile, {
    type: "object",
    example: { name: "Ada" },
    schema: { name: { type: "text" } },
  });
  const compiled = await compileDatabaseLens(captured, {
    resolveRange: async () => ({
      file: "source.ts",
      fromLine: 1,
      toLine: 1,
      lines: [{ number: 1, text: "source" }],
    }),
    changedLines: () => null,
  });
  assert.equal(
    compiled.useCases[0].operations[0].anchor.peek.resolution.source.lines[0].text,
    "source",
  );
});

test("all-surface browser output contains only interactive diagrams and source events", {
  timeout: 45_000,
}, async (t) => {
  const candidates = [
    process.env.CHROME_BIN,
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
  ].filter((candidate): candidate is string => Boolean(candidate));
  const chrome = candidates.find(existsSync);
  if (!chrome) return t.skip("Chrome executable is not installed");
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-cdp-"));
  const output = path.join(dir, "review.html");
  const profile = path.join(dir, "chrome-profile");
  const example = path.join(root, "skills/code-diagram/examples/loom-installer/review.mdx");
  const built = run(["build", example, "--repo", root, "--out", output]);
  assert.equal(built.status, 0, built.stderr);
  const browser = spawn(
    chrome,
    [
      "--headless=new",
      "--disable-gpu",
      "--no-sandbox",
      "--allow-file-access-from-files",
      "--remote-debugging-port=0",
      `--user-data-dir=${profile}`,
      pathToFileURL(output).href,
    ],
    { stdio: "ignore" },
  );
  let socket: WebSocket | undefined;
  try {
    const portFile = path.join(profile, "DevToolsActivePort");
    await waitFor(() => existsSync(portFile), "Chrome did not publish a DevTools port");
    const port = readFileSync(portFile, "utf8").split(/\r?\n/)[0];
    let page: { webSocketDebuggerUrl?: string } | undefined;
    for (let attempt = 0; attempt < 100 && !page?.webSocketDebuggerUrl; attempt += 1) {
      try {
        const targets = (await fetch(`http://127.0.0.1:${port}/json`).then((response) =>
          response.json(),
        )) as Array<{ type: string; webSocketDebuggerUrl?: string }>;
        page = targets.find((target) => target.type === "page");
      } catch {}
      if (!page?.webSocketDebuggerUrl) await delay(50);
    }
    assert.ok(page?.webSocketDebuggerUrl, "Chrome did not publish a page target");
    socket = new WebSocket(page.webSocketDebuggerUrl);
    const activeSocket = socket;
    await new Promise<void>((resolve, reject) => {
      activeSocket.addEventListener("open", () => resolve(), { once: true });
      activeSocket.addEventListener("error", () => reject(new Error("DevTools socket failed")), {
        once: true,
      });
    });
    type CdpMessage = {
      id?: number;
      method?: string;
      params?: {
        exceptionDetails?: { text?: string };
        entry?: { level?: string; text?: string };
        type?: string;
        args?: Array<{ value?: unknown; description?: string }>;
      };
      error?: unknown;
      result?: { exceptionDetails?: unknown; result?: { value?: unknown } };
    };
    let nextId = 1;
    const pending = new Map<number, (message: CdpMessage) => void>();
    const runtimeProblems: string[] = [];
    activeSocket.addEventListener("message", (event) => {
      const message = JSON.parse(String(event.data)) as CdpMessage;
      if (message.id) pending.get(message.id)?.(message);
      if (message.method === "Runtime.exceptionThrown")
        runtimeProblems.push(message.params?.exceptionDetails?.text ?? "browser exception");
      if (message.method === "Log.entryAdded" && message.params?.entry?.level === "error")
        runtimeProblems.push(message.params.entry.text ?? "browser log error");
      if (message.method === "Runtime.consoleAPICalled" && message.params?.type === "error")
        runtimeProblems.push(
          message.params.args
            ?.map((argument) => argument.value ?? argument.description)
            .join(" ") ?? "console error",
        );
    });
    const command = (method: string, params: Record<string, unknown> = {}) =>
      new Promise<CdpMessage>((resolve, reject) => {
        const id = nextId++;
        pending.set(id, (message) => {
          pending.delete(id);
          if (message.error) reject(new Error(JSON.stringify(message.error)));
          else resolve(message);
        });
        activeSocket.send(JSON.stringify({ id, method, params }));
      });
    const evaluate = async (expression: string) => {
      const message = await command("Runtime.evaluate", {
        expression,
        returnByValue: true,
        awaitPromise: true,
      });
      if (message.result?.exceptionDetails) throw new Error(JSON.stringify(message));
      return message.result?.result?.value;
    };
    await command("Runtime.enable");
    await command("Log.enable");
    await evaluate(`globalThis.__codeDiagramSourceEvents = [];
window.addEventListener("code-diagram:open-source", (event) => {
  globalThis.__codeDiagramSourceEvents.push(event.detail);
});`);
    await waitFor(
      async () =>
        (await evaluate("document.querySelectorAll('[data-code-diagram-kind] > *').length")) === 4,
      "Diagrams did not mount",
    );
    assert.equal(
      await evaluate(`[
        ".review-document",
        ".diagram-header",
        ".diagram-tour-overlay",
        ".comment-button",
        ".comment-hover-button",
        "[aria-label='Comment']",
        ".software-map-header",
        ".software-map-hotkeys-tab",
        ".react-flow__controls",
      ].every((selector) => !document.querySelector(selector))`),
      true,
    );
    assert.equal(
      await evaluate(`Boolean(
        document.querySelector('.sequence-diagram[data-diagram-design-type="sequence"] .sequence-message-dot')
        && document.querySelector('.call-stack-diff[data-diagram-design-type="tree"] .call-stack-row')
      )`),
      true,
    );
    await evaluate("document.querySelector('.sequence-message-dot').click()");
    assert.equal(
      await evaluate("globalThis.__codeDiagramSourceEvents.at(-1).sources[0].file"),
      "install.sh",
    );
    await waitFor(
      async () =>
        Boolean(
          await evaluate(
            "Boolean(document.querySelector('.database-lens .software-map-c4-edge-label--button'))",
          ),
        ),
      "Database lens did not finish layout",
    );
    assert.equal(
      await evaluate(`Boolean(
        document.querySelector('.database-lens[data-diagram-design-type="database-schema"] .software-map-data-store-schema-row')
      )`),
      true,
    );
    assert.equal(
      await evaluate(`(() => {
        const rows = [...document.querySelectorAll('.database-lens .software-map-data-store-schema-row')];
        const headers = [...document.querySelectorAll('.database-lens .software-map-data-store-schema-section-header')];
        const separated = (element) => {
          const [left, right] = element.children;
          if (!left || !right) return false;
          const leftBox = left.getBoundingClientRect();
          const rightBox = right.getBoundingClientRect();
          return rightBox.left - leftBox.right >= 1 && rightBox.right <= element.getBoundingClientRect().right;
        };
        return rows.length > 0 && headers.length > 0 && rows.every(separated) && headers.every(separated);
      })()`),
      true,
    );
    await evaluate(
      "document.querySelector('.database-lens .software-map-c4-edge-label--button').click()",
    );
    assert.equal(
      await evaluate("globalThis.__codeDiagramSourceEvents.at(-1).sources[0].file"),
      "install.sh",
    );
    await evaluate("document.querySelector('.call-stack-diff .call-stack-row').click()");
    assert.equal(
      await evaluate("globalThis.__codeDiagramSourceEvents.at(-1).sources[0].file"),
      "cli/loom/src/main.rs",
    );
    assert.equal(
      await evaluate("Boolean(document.querySelector('.code-diagram-source-panel'))"),
      false,
    );
    await waitFor(
      async () =>
        Boolean(
          await evaluate(
            "Boolean([...document.querySelectorAll('.software-map-c4-edge-label--button')].find((node) => node.textContent.includes('offers resources')))",
          ),
        ),
      "Software map did not finish layout",
    );
    assert.equal(
      await evaluate(`(() => {
        const map = document.querySelector('.software-map[data-diagram-design-type="architecture"]');
        const legend = map?.querySelector('.software-map-legend');
        return Boolean(
          map?.querySelector('.software-map-c4-edge-label')
          && legend?.textContent.includes('Call')
          && legend.textContent.includes('Dependency')
          && legend.textContent.includes('Foreign key')
        );
      })()`),
      true,
    );
    assert.equal(
      await evaluate(`(() => {
        const root = document.querySelector('.software-map');
        const labels = [...root.querySelectorAll('.software-map-c4-edge-label')];
        const nodes = [...root.querySelectorAll('.software-map-node')];
        const overlaps = (left, right) =>
          Math.min(left.right, right.right) - Math.max(left.left, right.left) > 0
          && Math.min(left.bottom, right.bottom) - Math.max(left.top, right.top) > 0;
        return labels.length > 0 && labels.every((label) => {
          const labelBox = label.getBoundingClientRect();
          return nodes.every((node) => !overlaps(labelBox, node.getBoundingClientRect()));
        });
      })()`),
      true,
    );
    await evaluate(
      "[...document.querySelectorAll('.software-map-c4-edge-label--button')].find((node) => node.textContent.includes('offers resources')).click()",
    );
    assert.equal(
      await evaluate("globalThis.__codeDiagramSourceEvents.at(-1).title"),
      "Relationship evidence",
    );
    await evaluate("document.querySelector('.software-map .software-map-c4-group-shell').click()");
    await delay(100);
    assert.equal(
      await evaluate("globalThis.__codeDiagramSourceEvents.at(-1).sources.length > 1"),
      true,
    );
    assert.equal(
      await evaluate("Boolean(document.querySelector('.code-diagram-source-panel'))"),
      false,
    );
    assert.deepEqual(runtimeProblems, []);
  } finally {
    socket?.close();
    browser.kill("SIGKILL");
  }
});

test("code-diagram keeps hostile MDX data and source text inert", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-escape-"));
  mkdirSync(path.join(dir, "src"));
  writeFileSync(
    path.join(dir, "src", "hostile.ts"),
    "</script><script>globalThis.pwned=true</script>\n",
  );
  const review = writeReview(
    dir,
    `import { defineActors, defineAnchors } from "virtual:progressive-review-authoring";
export const actors = defineActors({ a: { label: "A" }, b: { label: "B" } });
export const anchors = defineAnchors({ hostile: { title: "Hostile", peek: { file: "src/hostile.ts", fromLine: 1, toLine: 1 } } });
export const messages = [{ from: actors.a, to: actors.b, label: "<img src=x onerror=alert(1)>", anchor: anchors.hostile }];
`,
    `import { messages } from "./data.ts";

# Hostile

<SequenceDiagram label="<script>alert(1)</script>" messages={messages} />
`,
  );
  const output = path.join(dir, "hostile.html");
  const built = run(["build", review, "--repo", dir, "--out", output]);
  assert.equal(built.status, 0, built.stderr);
  const html = readFileSync(output, "utf8");
  assert.doesNotMatch(html, /<img src=x/);
  assert.doesNotMatch(html, /<script>alert\(1\)<\/script>/);
  assert.doesNotMatch(html, /<\/script><script>globalThis\.pwned/);
  assert.match(html, /\\u003cimg src=x/);
});
