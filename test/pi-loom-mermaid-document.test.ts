import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { accessSync, constants, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { transformMermaidForDocument } from "../plugins/pi-loom-mermaid/src/index.ts";

test("exports colored Mermaid with an explicit document fence", () => {
  const markdown = "Before\n\n```mermaid\nflowchart LR\n A:::red --> B:::green\n```\n\nAfter\n";
  const output = transformMermaidForDocument(markdown, 80);

  assert.match(output, /^Before\n\n```loom-mermaid\n/m);
  assert.ok(output.includes("\u001b[2;38;2;159;85;85m"));
  assert.ok(output.includes("\u001b[2;38;2;79;133;96m"));
  assert.ok(output.includes("\u001b[36m"));
  assert.doesNotMatch(output, /```text|```mermaid/);
  assert.match(output, /\n\nAfter\n$/);
});

test("document export preserves explicit styles but never emits terminal hyperlinks", () => {
  const markdown =
    '```mermaid\nflowchart LR\n A["日本 ` 38"]:::custom --> B:::orange\n classDef custom fill:#112233,stroke:#abcdef,color:#fedcba,font-weight:bold\n click A "https://example.com"\n```\n';
  const output = transformMermaidForDocument(markdown);
  assert.ok(output.includes("\u001b[1;38;2;171;205;239m"));
  assert.ok(output.includes("\u001b[1;38;2;254;220;186;48;2;17;34;51m"));
  assert.ok(output.includes("\u001b[2;38;2;154;116;56m"));
  assert.ok(!output.includes("\u001b]"));
});

test("packed CLI executes from node_modules and exports colored document fences", {
  skip: process.platform === "win32" && "Herdr document actions are macOS/Linux only",
}, () => {
  const dir = mkdtempSync(join(tmpdir(), "loom-mermaid-package-"));
  try {
    const packed = JSON.parse(
      execFileSync(
        "npm",
        [
          "pack",
          join(import.meta.dirname, "../plugins/pi-loom-mermaid"),
          "--pack-destination",
          dir,
          "--json",
          "--ignore-scripts",
        ],
        { encoding: "utf8" },
      ),
    );
    execFileSync(
      "npm",
      [
        "install",
        join(dir, packed[0].filename),
        "--ignore-scripts",
        "--legacy-peer-deps",
        "--no-audit",
        "--no-fund",
      ],
      { cwd: dir, stdio: "pipe" },
    );
    const cli = join(dir, "node_modules/.bin/loom-mermaid-render");
    accessSync(cli, constants.X_OK);
    const env: NodeJS.ProcessEnv = {
      ...process.env,
      BUN_INSTALL_CACHE_DIR: join(dir, "empty-bun-cache"),
    };
    delete env.NODE_PATH;
    delete env.BUN_INSTALL;
    const options = {
      cwd: dir,
      env,
      input: "```mermaid\nflowchart LR\n A:::red --> B:::green\n```\n",
      encoding: "utf8" as const,
    };
    const output = execFileSync("bun", ["--no-install", cli], options);
    assert.equal(execFileSync(cli, [], options), output);
    assert.match(output, /^```loom-mermaid\n/);
    assert.ok(output.includes("\u001b[2;38;2;159;85;85m"));
    assert.ok(output.includes("\u001b[2;38;2;79;133;96m"));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("keeps unsupported or oversized Mermaid source intact", () => {
  const unsupported = "```mermaid\nunsupported\n A --> B\n```\n";
  assert.equal(transformMermaidForDocument(unsupported), unsupported);
  const oversized = "```mermaid\nflowchart LR\n A --> B\n```\n";
  assert.equal(transformMermaidForDocument(oversized, 1), oversized);
  const ordinary = "```text\nnot Mermaid\n```\n";
  assert.equal(transformMermaidForDocument(ordinary), ordinary);
});
