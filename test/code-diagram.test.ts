import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { setTimeout as delay } from "node:timers/promises";
import { pathToFileURL } from "node:url";

const root = process.cwd();
const cli = path.join(root, "skills/code-diagram/scripts/code-diagram.ts");
const fixture = path.join(root, "skills/code-diagram/fixtures/review.mdx");

function run(args: string[]) {
  return spawnSync(process.execPath, ["--import", "tsx", cli, ...args], {
    cwd: root,
    encoding: "utf8",
  });
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
  assert.match(html, /const request = normalize\(input\)/);
  assert.match(html, /data-code-diagram-index="0"/);
  assert.doesNotMatch(html, /<script\s+[^>]*src=/i);
  assert.doesNotMatch(html, /<link\s+[^>]*href=/i);
  assert.doesNotMatch(html, /\/Users\/yassin\/projects\/personal\/review/);
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
  const evalResult = spawnSync(
    process.execPath,
    [
      "--import",
      "tsx",
      "--input-type=module",
      "-e",
      `
    import assert from "node:assert/strict";
    import { calls, createReviewDefinitionSession } from ${JSON.stringify(path.join(root, "skills/code-diagram/scripts/src/authoring.ts"))};
    import { diffCallStacks, patchChangedLines } from ${JSON.stringify(path.join(root, "skills/code-diagram/scripts/src/call-stack-diff.ts"))};
    import { createSurfaceRegistry } from ${JSON.stringify(path.join(root, "skills/code-diagram/scripts/src/diagram-registry.ts"))};
    import { defineSoftwareMap } from ${JSON.stringify(path.join(root, "skills/code-diagram/scripts/src/software-map-model.ts"))};
    import { projectInlineC4, collapseInlineC4Node } from ${JSON.stringify(path.join(root, "skills/code-diagram/scripts/src/software-map-c4-projection.ts"))};
    import { layoutInlineC4 } from ${JSON.stringify(path.join(root, "skills/code-diagram/scripts/src/software-map-c4-layout.ts"))};
    const registry = createSurfaceRegistry(${JSON.stringify(path.join(root, "skills/code-diagram/scripts/src/software-map-model.ts"))});
    assert.deepEqual(registry.map((surface) => surface.kind), ["sequence", "call-stack-diff", "database-lens", "software-map"]);
    const session = createReviewDefinitionSession({});
    const anchors = session.defineAnchors({ parent: { title: "Parent", peek: { file: "parent.ts", fromLine: 1, toLine: 1 } }, child: { title: "Child", peek: { file: "child.ts", fromLine: 1, toLine: 1 } } });
    const assertion = calls(anchors.parent, anchors.child, "queue");
    assert.equal(assertion.parent, anchors.parent); assert.equal(assertion.child, anchors.child);
    assert.deepEqual(diffCallStacks([anchors.parent], [anchors.child]).map((row) => row.change), ["removed", "added"]);
    const lines = patchChangedLines("@@ -3,1 +3,1 @@\\n-old\\n+new"); assert.deepEqual([...lines.deleted], [3]); assert.deepEqual([...lines.added], [3]);
    const map = defineSoftwareMap({ systems: { loom: { containers: { cli: { components: { planner: {} } } } } } }); assert.ok(map.elementsByPath.has("loom.cli.planner"));
    const mapped = defineSoftwareMap({ systems: { app: { dataStores: { db: { kind: "database", tables: { users: { schema: { id: { type: "text", pk: true } } } } } } } } });
    const mappedActors = session.defineSoftwareActors(mapped, { app: "app" }); assert.equal(mappedActors.app.softwareMapPath, "app");
    const mappedStores = session.defineSoftwareStores(mapped, { db: { path: "app.db" } }); assert.equal(mappedStores.db.tables.users.id.__kind, undefined); assert.equal(mappedStores.db.softwareMapPath, "app.db");
    const projection = projectInlineC4({ model: map, expandedNodeIds: new Set(["loom", "loom.cli"]) });
    assert.deepEqual(projection.nodes.map((node) => node.id), ["loom", "loom.cli", "loom.cli.planner"]);
    const layout = layoutInlineC4({ nodes: projection.nodes, relationships: projection.relationships, expandedIds: projection.expandedNodeIds });
    assert.equal(layout.nodeBboxes.size, 3); assert.ok(layout.groupBboxes.has("loom"));
    assert.deepEqual([...collapseInlineC4Node(new Set(["loom", "loom.cli"]), "loom")], []);
  `,
    ],
    { cwd: root, encoding: "utf8" },
  );
  assert.equal(evalResult.status, 0, evalResult.stderr);
});

test("vendored renderers are byte-identical to the pinned Review sources", () => {
  const expected = new Map([
    ["diagrams.tsx", "7e4af3b9327e9fca14f50618bc8fbc39282b3f2509b36a2aa3ce7a55c9ca46f4"],
    ["call-stack-diff.tsx", "0cf34de0737999b1c8211d33ba201b1f3f143dfc641d2857fbf45118a6f1f3c5"],
    ["database-lens.tsx", "746dde967df63dd716c7001fe2e1bc67084fd103dae9edf422c1b89ea858dbcc"],
    [
      "software-map/SoftwareMap.tsx",
      "22d5f5eca647c2a2e2e8ff6e88f79946076cba234773bbdef4cbaef3211debc4",
    ],
    [
      "software-map/c4-projection.ts",
      "da8dfa3285fe1afbd3b982b8f30b839e9f40fbc763c5667aff63297c50e37479",
    ],
    [
      "software-map/c4-layout.ts",
      "0799902bdead82123dbdd947ae49813579794da3496ac4573780d9090a1080b8",
    ],
  ]);
  const runtime = path.join(root, "skills/code-diagram/scripts/src/review-runtime/app/src");
  for (const [file, hash] of expected) {
    const actual = createHash("sha256")
      .update(readFileSync(path.join(runtime, file)))
      .digest("hex");
    assert.equal(actual, hash, file);
  }
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
  assert.doesNotMatch(html, /<(?:script|link)[^>]+(?:src|href)=/i);
  assert.doesNotMatch(html, /\/Users\/yassin\/projects\/personal\/review/);
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

test("DatabaseLens browser model preserves exact schemas and tour evidence", () => {
  const result = spawnSync(
    process.execPath,
    [
      "--import",
      "tsx",
      "--input-type=module",
      "-e",
      `
import assert from "node:assert/strict";
import React from "react";
import { createReviewDefinitionSession } from ${JSON.stringify(path.join(root, "skills/code-diagram/scripts/src/authoring.ts"))};
import { createDatabaseLensComponents, compileDatabaseLens } from ${JSON.stringify(path.join(root, "skills/code-diagram/scripts/src/database-lens-model.ts"))};
const session = createReviewDefinitionSession({});
const stores = session.defineStores({ mixed: { kind: "relational", label: "Mixed", tables: { users: { schema: { id: { type: "text", pk: true }, profile: { type: "object", example: { name: "Ada" }, schema: { name: { type: "text" } } } } } }, documents: { audit: { schema: { actor_id: { type: "text", fk: "users.id" } } } } } });
const actors = session.defineActors({ api: { label: "API" } });
const anchors = session.defineAnchors({ read: { title: "Read", peek: { file: "source.ts", fromLine: 1, toLine: 1 } } });
let captured;
const components = createDatabaseLensComponents((model) => { captured = model; return React.createElement("div"); });
components.DatabaseLens({ stores, children: React.createElement(components.DbUseCase, { id: "read", label: "Read", children: React.createElement(components.DbRead, { from: stores.mixed.tables.users.profile.name, to: actors.api, label: "read", anchor: anchors.read }) }) });
assert.deepEqual(captured.stores[0].collections.map((collection) => collection.kind), ["tables", "documents"]);
assert.deepEqual(captured.stores[0].collections[0].schema.profile, { type: "object", example: { name: "Ada" }, schema: { name: { type: "text" } } });
const compiled = await compileDatabaseLens(captured, { resolveRange: async () => ({ file: "source.ts", fromLine: 1, toLine: 1, lines: [{ number: 1, text: "source" }] }), changedLines: () => null });
assert.equal(compiled.useCases[0].operations[0].anchor.peek.resolution.source.lines[0].text, "source");
`,
    ],
    { cwd: root, encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
});

test("generated all-surface HTML mounts in headless Chrome", { timeout: 30_000 }, (t) => {
  const candidates = [
    process.env.CHROME_BIN,
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
  ].filter((candidate): candidate is string => Boolean(candidate));
  const chrome = candidates.find(existsSync);
  if (!chrome) return t.skip("Chrome executable is not installed");
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-browser-"));
  const output = path.join(dir, "review.html");
  const example = path.join(root, "skills/code-diagram/examples/loom-installer/review.mdx");
  const built = run(["build", example, "--repo", root, "--out", output]);
  assert.equal(built.status, 0, built.stderr);
  const browser = spawnSync(
    chrome,
    [
      "--headless=new",
      "--disable-gpu",
      "--no-sandbox",
      "--allow-file-access-from-files",
      "--virtual-time-budget=8000",
      "--dump-dom",
      pathToFileURL(output).href,
    ],
    { encoding: "utf8", maxBuffer: 10 * 1024 * 1024 },
  );
  assert.equal(browser.status, 0, browser.stderr);
  assert.match(browser.stdout, /class="sequence-diagram/);
  assert.match(browser.stdout, /class="call-stack-diff/);
  assert.match(browser.stdout, /class="database-lens/);
  assert.match(browser.stdout, /class="software-map(?:\s|")/);
  assert.match(browser.stdout, /class="software-map-c4-canvas/);
  assert.match(browser.stdout, />Install mise</);
  assert.match(browser.stdout, />Embedded catalog</);
  assert.doesNotMatch(browser.stdout, /<p>Source evidence unavailable\.<\/p>/);
});

test("all-surface browser interactions keep tours, map expansion, and multi-range evidence", {
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
    for (let attempt = 0; attempt < 100 && !existsSync(portFile); attempt += 1) await delay(50);
    assert.ok(existsSync(portFile), "Chrome did not publish a DevTools port");
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
    for (let attempt = 0; attempt < 100; attempt += 1) {
      if (await evaluate("document.querySelectorAll('[data-code-diagram-kind] > *').length === 4"))
        break;
      await delay(50);
    }
    assert.equal(
      await evaluate("document.querySelectorAll('[data-code-diagram-kind] > *').length"),
      4,
    );
    await evaluate("document.querySelector('.sequence-diagram .diagram-tour-button').click()");
    await delay(100);
    assert.equal(
      await evaluate("Boolean(document.querySelector('.diagram-tour-overlay .code-peek-file'))"),
      true,
    );
    await evaluate(
      "document.querySelector('.diagram-tour-overlay [aria-label=\"Next step\"]').click()",
    );
    assert.equal(
      await evaluate(
        "document.querySelector('.diagram-tour-overlay .tour-pill-count').textContent",
      ),
      "2/7",
    );
    await evaluate("document.querySelector('[aria-label=\"Close guided tour\"]').click()");
    await evaluate("document.querySelector('.database-lens .diagram-tour-button').click()");
    await delay(100);
    assert.equal(
      await evaluate("Boolean(document.querySelector('.diagram-tour-overlay .code-peek-file'))"),
      true,
    );
    assert.equal(
      await evaluate(
        "Boolean([...document.querySelectorAll('.diagram-tour-overlay p')].find((node) => node.textContent === 'Source evidence unavailable.'))",
      ),
      false,
    );
    await evaluate(
      "document.querySelector('.diagram-tour-overlay [aria-label=\"Next step\"]').click()",
    );
    assert.equal(
      await evaluate(
        "document.querySelector('.diagram-tour-overlay .tour-pill-count').textContent",
      ),
      "2/2",
    );
    await evaluate("document.querySelector('[aria-label=\"Close guided tour\"]').click()");
    await evaluate("document.querySelector('.call-stack-diff .call-stack-row').click()");
    assert.equal(
      await evaluate("globalThis.__codeDiagramSourceEvents.at(-1).sources[0].file"),
      "cli/loom/src/main.rs",
    );
    assert.equal(
      await evaluate("Boolean(document.querySelector('.code-diagram-source-panel'))"),
      false,
    );
    await evaluate(
      "document.querySelector('.software-map [aria-label=\"Expand software map\"]').click()",
    );
    await delay(100);
    assert.equal(
      await evaluate("document.querySelectorAll('.software-map > .software-map-frame').length"),
      1,
    );
    assert.equal(
      await evaluate(
        "Boolean(document.querySelector('.software-map-overlay .software-map-frame'))",
      ),
      true,
    );
    assert.equal(
      await evaluate("document.querySelector('.software-map-overlay').getAttribute('role')"),
      "dialog",
    );
    assert.equal(
      await evaluate("Boolean(document.activeElement.closest('.software-map-overlay'))"),
      true,
    );
    await evaluate(
      "document.querySelector('[aria-label=\"Close expanded software map\"]').click()",
    );
    assert.equal(await evaluate("Boolean(document.querySelector('.software-map-overlay'))"), false);
    assert.equal(
      await evaluate("document.querySelectorAll('.software-map > .software-map-frame').length"),
      1,
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
