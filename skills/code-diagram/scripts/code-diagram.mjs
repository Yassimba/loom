#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { CodeDiagramInputError, normalizeDocument } from "./authoring.mjs";

const HELP = `code-diagram — Review-style offline code diagrams

Sequence Diagram is the only supported surface in this preview; Review parity is not yet complete.

Usage:
  code-diagram check <document.mjs|document.json> [--repo <path>]
  code-diagram build <document.mjs|document.json> [--repo <path>] --out <document.html>
`;

main().catch((error) => {
  if (error instanceof CodeDiagramInputError) {
    console.error(`${error.code}: ${error.message}`);
  } else {
    console.error(error instanceof Error ? error.message : String(error));
  }
  process.exitCode = 1;
});

async function main() {
  const parsed = parseArgs(process.argv.slice(2));
  if (parsed.help) {
    process.stdout.write(HELP);
    return;
  }
  const repo = await realpath(parsed.repo);
  const documentPath = path.resolve(parsed.input);
  const document = normalizeDocument(await loadInput(documentPath));
  const compiled = await compileDocument(document, repo);
  if (parsed.command === "check") {
    process.stderr.write(`checked ${compiled.diagrams.length} sequence diagram(s)\n`);
    return;
  }
  const output = path.resolve(parsed.out);
  await mkdir(path.dirname(output), { recursive: true });
  await writeFile(output, await renderHtml(compiled), "utf8");
  process.stderr.write(`wrote ${output}\n`);
}

function parseArgs(args) {
  if (args.length === 0 || args.includes("--help") || args.includes("-h")) return { help: true };
  const command = args.shift();
  if (command !== "check" && command !== "build") throw new Error(`unknown command ${JSON.stringify(command)}\n\n${HELP}`);
  const input = args.shift();
  if (!input || input.startsWith("-")) throw new Error(`missing document path\n\n${HELP}`);
  let repo = process.cwd();
  let out;
  while (args.length > 0) {
    const flag = args.shift();
    const value = args.shift();
    if (!value) throw new Error(`missing value for ${flag}`);
    if (flag === "--repo") repo = path.resolve(value);
    else if (flag === "--out") out = value;
    else throw new Error(`unknown option ${flag}`);
  }
  if (command === "build" && !out) throw new Error(`build requires --out\n\n${HELP}`);
  return { help: false, command, input, repo, out };
}

async function loadInput(file) {
  if (path.extname(file) === ".json") return JSON.parse(await readFile(file, "utf8"));
  const module = await import(`${pathToFileURL(file).href}?code-diagram=${Date.now()}`);
  if (!("default" in module)) throw new Error(`${file} must export the document as default`);
  return module.default;
}

async function compileDocument(document, repo) {
  const repoPrefix = `${repo}${path.sep}`;
  const diagrams = [];
  for (const diagram of document.diagrams) {
    const messages = [];
    for (const message of diagram.messages) {
      let source;
      if (message.evidence) {
        const candidate = path.resolve(repo, message.evidence.file);
        if (candidate !== repo && !candidate.startsWith(repoPrefix)) {
          throw new CodeDiagramInputError("EVIDENCE_FILE_INVALID", message.evidence.file, "resolves outside repository");
        }
        let resolved;
        try {
          resolved = await realpath(candidate);
        } catch {
          throw new CodeDiagramInputError("EVIDENCE_FILE_MISSING", message.evidence.file, "file does not exist");
        }
        if (resolved !== repo && !resolved.startsWith(repoPrefix)) {
          throw new CodeDiagramInputError("EVIDENCE_FILE_INVALID", message.evidence.file, "symlink resolves outside repository");
        }
        const lines = (await readFile(resolved, "utf8")).split(/\r\n|\n|\r/);
        if (message.evidence.toLine > lines.length) {
          throw new CodeDiagramInputError(
            "EVIDENCE_RANGE_INVALID",
            message.evidence.file,
            `range ${message.evidence.fromLine}-${message.evidence.toLine} exceeds ${lines.length} lines`,
          );
        }
        source = {
          ...message.evidence,
          lines: lines
            .slice(message.evidence.fromLine - 1, message.evidence.toLine)
            .map((text, index) => ({ number: message.evidence.fromLine + index, text })),
        };
      }
      messages.push({ ...message, source });
    }
    diagrams.push({ ...diagram, messages });
  }
  return {
    ...document,
    repo: path.basename(repo),
    revision: resolveRevision(repo),
    diagrams,
  };
}

function resolveRevision(repo) {
  try {
    return execFileSync("git", ["-C", repo, "rev-parse", "HEAD"], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
  } catch {
    return null;
  }
}

async function renderHtml(compiled) {
  const assets = path.dirname(fileURLToPath(import.meta.url));
  const [viewer, styles] = await Promise.all([
    readFile(path.join(assets, "viewer.js"), "utf8"),
    readFile(path.join(assets, "viewer.css"), "utf8"),
  ]);
  const model = JSON.stringify(compiled).replaceAll("<", "\\u003c").replaceAll("\u2028", "\\u2028").replaceAll("\u2029", "\\u2029");
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; font-src data:; style-src 'unsafe-inline'; script-src 'unsafe-inline'">
<title>${escapeHtml(compiled.title)}</title>
<style>${styles.replaceAll("</style", "<\\/style")}</style>
</head>
<body>
<div id="code-diagram-root"></div>
<script>globalThis.__CODE_DIAGRAM_DOCUMENT__=${model};</script>
<script>${viewer.replaceAll("</script", "<\\/script")}</script>
</body>
</html>\n`;
}

function escapeHtml(value) {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}
