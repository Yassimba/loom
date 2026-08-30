import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

const root = process.cwd();
const cli = path.join(root, "skills/code-diagram/scripts/code-diagram.mjs");
const fixture = path.join(root, "skills/code-diagram/fixtures/sequence.mjs");

function run(args) {
  return spawnSync(process.execPath, [cli, ...args], { cwd: root, encoding: "utf8" });
}

test("code-diagram builds one self-contained Review-style sequence document", () => {
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
  assert.doesNotMatch(html, /<script\s+[^>]*src=/i);
  assert.doesNotMatch(html, /<link\s+[^>]*href=/i);
  assert.doesNotMatch(html, /\/Users\/yassin\/projects\/personal\/review/);
});

test("code-diagram rejects unsupported surfaces and stale evidence", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-invalid-"));
  const unsupported = path.join(dir, "unsupported.json");
  writeFileSync(
    unsupported,
    JSON.stringify({
      version: 1,
      title: "No",
      diagrams: [{ type: "state", label: "No", actors: {}, messages: [] }],
    }),
  );
  const unsupportedResult = run(["check", unsupported, "--repo", root]);
  assert.notEqual(unsupportedResult.status, 0);
  assert.match(unsupportedResult.stderr, /DIAGRAM_UNSUPPORTED/);

  const stale = path.join(dir, "stale.json");
  writeFileSync(
    stale,
    JSON.stringify({
      version: 1,
      title: "Stale",
      diagrams: [
        {
          type: "sequence",
          label: "Stale",
          actors: { a: { label: "A" }, b: { label: "B" } },
          messages: [
            {
              from: "a",
              to: "b",
              label: "missing",
              evidence: {
                file: "skills/code-diagram/fixtures/source.ts",
                fromLine: 999,
                toLine: 999,
              },
            },
          ],
        },
      ],
    }),
  );
  const staleResult = run(["check", stale, "--repo", root]);
  assert.notEqual(staleResult.status, 0);
  assert.match(staleResult.stderr, /EVIDENCE_RANGE_INVALID/);
});

test("code-diagram keeps hostile labels and source text inert", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-escape-"));
  mkdirSync(path.join(dir, "src"));
  writeFileSync(
    path.join(dir, "src", "hostile.ts"),
    "</script><script>globalThis.pwned=true</script>\n",
  );
  const input = path.join(dir, "hostile.json");
  const output = path.join(dir, "hostile.html");
  writeFileSync(
    input,
    JSON.stringify({
      version: 1,
      title: "Hostile",
      diagrams: [
        {
          type: "sequence",
          label: "<img src=x onerror=alert(1)>",
          actors: { a: { label: "A" }, b: { label: "B" } },
          messages: [
            {
              from: "a",
              to: "b",
              label: "<script>alert(1)</script>",
              evidence: { file: "src/hostile.ts", fromLine: 1, toLine: 1 },
            },
          ],
        },
      ],
    }),
  );
  const built = run(["build", input, "--repo", dir, "--out", output]);
  assert.equal(built.status, 0, built.stderr);
  const html = readFileSync(output, "utf8");
  assert.doesNotMatch(html, /<img src=x/);
  assert.doesNotMatch(html, /<script>alert\(1\)<\/script>/);
  assert.doesNotMatch(html, /<\/script><script>globalThis\.pwned/);
  assert.match(html, /\\u003cimg src=x/);
});

test("committed code-diagram viewer bundle is current", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "code-diagram-bundle-"));
  const built = spawnSync(process.execPath, ["scripts/build-code-diagram.mjs", "--outdir", dir], {
    cwd: root,
    encoding: "utf8",
  });
  assert.equal(built.status, 0, `${built.stdout}\n${built.stderr}`);
  for (const file of ["viewer.js", "viewer.css"]) {
    assert.deepEqual(
      readFileSync(path.join(dir, file)),
      readFileSync(path.join(root, "skills/code-diagram/scripts", file)),
      `${file} is stale; run npm run build:code-diagram`,
    );
  }
});
