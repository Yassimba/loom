import { spawnSync } from "node:child_process";
import { access, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { compile } from "@mdx-js/mdx";
import { build, type Message, type Plugin } from "esbuild";
import remarkFrontmatter from "remark-frontmatter";
import remarkGfm from "remark-gfm";

import {
  compiledDocumentSchema,
  type CompiledDocument,
  type CompiledSurface,
} from "./src/diagram-family";
import { compileSurfaces, createSurfaceRegistry } from "./src/diagram-registry";
import { EvidenceInputError, SourceEvidenceService } from "./src/source-evidence";

const scriptsDir = path.dirname(fileURLToPath(import.meta.url));
const packageRoot = path.resolve(scriptsDir, "../../..");
const authoringSource = path.join(scriptsDir, "src", "authoring.ts");
const virtualAuthoringSource = path.join(scriptsDir, "src", "virtual-authoring.ts");
const registrySource = path.join(scriptsDir, "src", "diagram-registry.ts");
const softwareMapModelSource = path.join(scriptsDir, "src", "software-map-model.ts");
const viewerSource = path.join(scriptsDir, "src", "viewer.tsx");

const HELP = `code-diagram — Review document to offline HTML

Supported Review surfaces: SequenceDiagram, CallStackDiff, DatabaseLens, and an optional adjacent software-map.ts.

Usage:
  code-diagram check <review.mdx> [--repo <path>]
  code-diagram build <review.mdx> [--repo <path>] --out <document.html>
`;

class CodeDiagramInputError extends Error {
  constructor(
    readonly code: string,
    subject: string,
    detail: string,
  ) {
    super(`${subject}: ${detail}`);
  }
}
type ParsedArgs =
  | { help: true }
  | { help: false; command: "check"; input: string; repo: string }
  | { help: false; command: "build"; input: string; repo: string; out: string };
type AuthoredDocument = { title: string; html: string; diagrams: CompiledSurface[] };

main().catch((error) => {
  if (error instanceof CodeDiagramInputError || error instanceof EvidenceInputError)
    console.error(`${error.code}: ${error.message}`);
  else console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});

async function main() {
  const parsed = parseArgs(process.argv.slice(2));
  if (parsed.help) {
    process.stdout.write(HELP);
    return;
  }
  const repo = await realpath(parsed.repo);
  const reviewPath = path.resolve(parsed.input);
  if (path.basename(reviewPath) !== "review.mdx")
    throw new CodeDiagramInputError(
      "DOCUMENT_NAME_INVALID",
      reviewPath,
      "Review documents are named review.mdx",
    );
  const directory = path.dirname(reviewPath);
  const dataPath = path.join(directory, "data.ts");
  const mapPath = path.join(directory, "software-map.ts");
  const hasMap = await exists(mapPath);
  await typecheckAuthoring(dataPath, hasMap ? mapPath : null, repo);
  const registry = createSurfaceRegistry(softwareMapModelSource);
  const authored = await evaluateReviewDocument(reviewPath, repo);
  for (const descriptor of registry) {
    if (descriptor.source.type !== "artifact") continue;
    const artifact = path.join(directory, descriptor.source.fileName);
    if (!(await exists(artifact))) continue;
    for (const model of await descriptor.source.collect(artifact, repo)) {
      const index =
        authored.diagrams.push({
          kind: descriptor.kind,
          model: descriptor.capturedSchema.parse(model),
        }) - 1;
      authored.html += `<div data-code-diagram-kind="${descriptor.kind}" data-code-diagram-index="${index}"></div>`;
    }
  }
  validateSoftwareMapReferences(authored.diagrams);
  validateInertAuthoredHtml(authored.html, reviewPath);
  const evidence = await SourceEvidenceService.create(repo);
  const diagrams = await compileSurfaces(authored.diagrams, registry, evidence);
  const compiled = compiledDocumentSchema.parse({
    version: 1,
    title: authored.title,
    html: authored.html,
    repo: path.basename(repo),
    revision: evidence.revision,
    diagrams,
  });
  assertJsonSafe(compiled);
  if (parsed.command === "check") {
    const countByKind = new Map<string, number>();
    for (const diagram of compiled.diagrams)
      countByKind.set(diagram.kind, (countByKind.get(diagram.kind) ?? 0) + 1);
    const counts = [...countByKind].map(([kind, count]) => `${count} ${kind}`).join(", ");
    process.stderr.write(`checked ${compiled.diagrams.length} surface(s): ${counts}\n`);
    return;
  }
  const output = path.resolve(parsed.out);
  await mkdir(path.dirname(output), { recursive: true });
  await writeFile(output, await renderHtml(compiled, registry), "utf8");
  process.stderr.write(`wrote ${output}\n`);
}

function parseArgs(args: string[]): ParsedArgs {
  if (!args.length || args.includes("--help") || args.includes("-h")) return { help: true };
  const command = args.shift();
  if (command !== "check" && command !== "build")
    throw new Error(`unknown command ${JSON.stringify(command)}\n\n${HELP}`);
  const input = args.shift();
  if (!input || input.startsWith("-")) throw new Error(`missing review.mdx path\n\n${HELP}`);
  let repo = process.cwd();
  let out: string | undefined;
  while (args.length) {
    const flag = args.shift();
    const value = args.shift();
    if (!value) throw new Error(`missing value for ${flag}`);
    if (flag === "--repo") repo = path.resolve(value);
    else if (flag === "--out") out = value;
    else throw new Error(`unknown option ${flag}`);
  }
  if (command === "build") {
    if (!out) throw new Error(`build requires --out\n\n${HELP}`);
    return { help: false, command, input, repo, out };
  }
  return { help: false, command, input, repo };
}

async function typecheckAuthoring(dataPath: string, mapPath: string | null, repo: string) {
  const temp = await mkdtemp(path.join(tmpdir(), "code-diagram-tsc-"));
  const config = path.join(temp, "tsconfig.json");
  await writeFile(
    config,
    JSON.stringify({
      compilerOptions: {
        strict: true,
        noEmit: true,
        skipLibCheck: true,
        target: "ES2022",
        module: "ESNext",
        moduleResolution: "Bundler",
        jsx: "react-jsx",
        paths: {
          "virtual:progressive-review-authoring": [virtualAuthoringSource],
          "@dev.fast/progressive-review/software-map-model": [softwareMapModelSource],
        },
      },
      files: [dataPath, ...(mapPath ? [mapPath] : [])],
    }),
  );
  const tsc = path.join(packageRoot, "node_modules", "typescript", "bin", "tsc");
  const result = spawnSync(process.execPath, [tsc, "-p", config], { cwd: repo, encoding: "utf8" });
  await rm(temp, { recursive: true, force: true });
  if (result.status !== 0)
    throw new CodeDiagramInputError(
      "TYPESCRIPT_INVALID",
      dataPath,
      `failed type checking\n${result.stdout}${result.stderr}`,
    );
}

async function evaluateReviewDocument(reviewPath: string, repo: string): Promise<AuthoredDocument> {
  const entry = `
    import React from "react";
    import { renderToStaticMarkup } from "react-dom/server";
    import ReviewDocument from ${JSON.stringify(reviewPath)};
    import { __reviewDefinitionsReady } from "virtual:progressive-review-authoring";
    import { createMdxComponents, createSurfaceRegistry } from ${JSON.stringify(registrySource)};
    export async function renderReviewDocument() {
      await __reviewDefinitionsReady();
      const diagrams = [];
      const components = createMdxComponents(createSurfaceRegistry(${JSON.stringify(softwareMapModelSource)}), (kind, model) => {
        const index = diagrams.push({ kind, model }) - 1;
        return React.createElement("div", { "data-code-diagram-kind": kind, "data-code-diagram-index": index });
      });
      return { html: renderToStaticMarkup(React.createElement(ReviewDocument, { components })), diagrams };
    }
  `;
  let result;
  try {
    result = await build({
      absWorkingDir: repo,
      bundle: true,
      format: "esm",
      platform: "node",
      packages: "external",
      target: ["node22"],
      write: false,
      stdin: {
        contents: entry,
        loader: "ts",
        resolveDir: repo,
        sourcefile: "code-diagram-review-entry.ts",
      },
      plugins: [reviewDocumentPlugin()],
      logLevel: "silent",
    });
  } catch (error) {
    throw new CodeDiagramInputError("DOCUMENT_COMPILE_INVALID", reviewPath, esbuildMessage(error));
  }
  const code = result.outputFiles[0]?.text;
  if (!code) throw new Error("Review document compiler produced no output");
  const temp = await mkdtemp(path.join(packageRoot, ".code-diagram-eval-"));
  const modulePath = path.join(temp, "review-document.mjs");
  try {
    await writeFile(modulePath, code);
    const loaded = await import(`${pathToFileURL(modulePath).href}?${Date.now()}`);
    const rendered = await loaded.renderReviewDocument();
    const source = await readFile(reviewPath, "utf8");
    const headings = [...source.matchAll(/^#\s+(.+)$/gm)];
    if (headings.length !== 1)
      throw new CodeDiagramInputError(
        "DOCUMENT_TITLE_INVALID",
        reviewPath,
        "must contain exactly one Markdown H1",
      );
    return { title: headings[0]![1]!.trim(), html: rendered.html, diagrams: rendered.diagrams };
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
}

function reviewDocumentPlugin(): Plugin {
  return {
    name: "review-document",
    setup(pluginBuild) {
      pluginBuild.onResolve({ filter: /^virtual:progressive-review-authoring$/ }, () => ({
        path: virtualAuthoringSource,
      }));
      pluginBuild.onResolve(
        { filter: /^@dev\.fast\/progressive-review\/software-map-model$/ },
        () => ({ path: softwareMapModelSource }),
      );
      pluginBuild.onLoad({ filter: /\.mdx$/ }, async ({ path: mdxPath }) => ({
        contents: String(
          await compile(await readFile(mdxPath, "utf8"), {
            development: false,
            jsx: false,
            outputFormat: "program",
            remarkPlugins: [remarkFrontmatter, remarkGfm],
          }),
        ),
        loader: "js",
        resolveDir: path.dirname(mdxPath),
      }));
    },
  };
}

async function renderHtml(
  compiled: CompiledDocument,
  registry: ReturnType<typeof createSurfaceRegistry>,
): Promise<string> {
  const imports: string[] = [
    `import { mountReviewDocument } from ${JSON.stringify(viewerSource)};`,
  ];
  const entries: string[] = [];
  registry.forEach((descriptor, index) => {
    imports.push(
      `import { ${descriptor.browser.schemaExport} as schema${index} } from ${JSON.stringify(path.resolve(scriptsDir, descriptor.browser.schemaSpecifier))};`,
    );
    imports.push(
      `import { ${descriptor.browser.rendererExport} as Renderer${index} } from ${JSON.stringify(path.resolve(scriptsDir, descriptor.browser.rendererSpecifier))};`,
    );
    entries.push(
      `${JSON.stringify(descriptor.kind)}: { schema: schema${index}, Renderer: Renderer${index} }`,
    );
  });
  const entry = `${imports.join("\n")}\nmountReviewDocument(globalThis.__CODE_DIAGRAM_DOCUMENT__, {${entries.join(",")}});`;
  const bundle = await build({
    absWorkingDir: packageRoot,
    bundle: true,
    format: "iife",
    platform: "browser",
    target: ["chrome120", "firefox121", "safari17"],
    stdin: {
      contents: entry,
      loader: "tsx",
      resolveDir: scriptsDir,
      sourcefile: "generated-code-diagram-viewer.tsx",
    },
    outdir: "code-diagram-viewer",
    jsx: "automatic",
    define: { "process.env.NODE_ENV": '"production"' },
    legalComments: "inline",
    minify: true,
    write: false,
    logLevel: "silent",
  });
  const viewer = bundle.outputFiles.find((file) => file.path.endsWith(".js"))?.text;
  const styles = bundle.outputFiles.find((file) => file.path.endsWith(".css"))?.text;
  if (!viewer || !styles) throw new Error("Viewer compiler produced incomplete assets");
  const model = JSON.stringify(compiled)
    .replaceAll("<", "\\u003c")
    .replaceAll("\u2028", "\\u2028")
    .replaceAll("\u2029", "\\u2029");
  const libavoidWasm = await readFile(
    path.join(packageRoot, "node_modules", "@mr_mint", "elkjs-libavoid", "dist", "libavoid.wasm"),
  );
  const wasmBootstrap = `globalThis.__CODE_DIAGRAM_LIBAVOID_WASM_URL__=URL.createObjectURL(new Blob([Uint8Array.from(atob("${libavoidWasm.toString("base64")}"),c=>c.charCodeAt(0))],{type:"application/wasm"}));`;
  return `<!doctype html>\n<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; font-src data:; connect-src blob: data:; worker-src blob:; style-src 'unsafe-inline'; script-src 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval'"><title>${escapeHtml(compiled.title)}</title><style>${styles.replaceAll("</style", "<\\/style")}</style></head><body><main class="review-canvas-root code-diagram-page"><article class="review-document code-diagram-document"><div class="code-diagram-revision">${escapeHtml(compiled.repo)}${compiled.revision ? ` · ${escapeHtml(compiled.revision.slice(0, 12))}` : ""}</div>${compiled.html}</article></main><script>${wasmBootstrap}</script><script>globalThis.__CODE_DIAGRAM_DOCUMENT__=${model};</script><script>${viewer.replaceAll("</script", "<\\/script")}</script></body></html>\n`;
}

function validateSoftwareMapReferences(diagrams: CompiledSurface[]) {
  const map = diagrams.find((diagram) => diagram.kind === "software-map")?.model as
    | { elements?: Array<{ path?: unknown }> }
    | undefined;
  const knownPaths = new Set(
    map?.elements?.flatMap((element) =>
      typeof element.path === "string" ? [element.path] : [],
    ) ?? [],
  );
  const references = new Set<string>();
  const visit = (value: unknown): void => {
    if (!value || typeof value !== "object") return;
    if (Array.isArray(value)) {
      for (const item of value) visit(item);
      return;
    }
    for (const [key, child] of Object.entries(value)) {
      if (
        (key === "softwareMapPath" || key === "storeSoftwareMapPath") &&
        typeof child === "string"
      )
        references.add(child);
      visit(child);
    }
  };
  for (const diagram of diagrams) if (diagram.kind !== "software-map") visit(diagram.model);
  if (!references.size) return;
  if (!map)
    throw new Error(
      "SOFTWARE_MAP_REFERENCE_INVALID: authored softwareMapPath values require adjacent software-map.ts",
    );
  const unknown = [...references].filter((path) => !knownPaths.has(path));
  if (unknown.length)
    throw new Error(
      `SOFTWARE_MAP_REFERENCE_INVALID: unknown software-map path(s): ${unknown.join(", ")}`,
    );
}

function validateInertAuthoredHtml(html: string, inputPath: string) {
  const executable = /<(?:script|iframe|object|embed|base|meta|link)\b|\bjavascript\s*:/i.exec(html);
  if (executable)
    throw new CodeDiagramInputError(
      "DOCUMENT_HTML_UNSAFE",
      inputPath,
      `executable authored HTML is not allowed (${executable[0]})`,
    );
}

function assertJsonSafe(value: unknown, pathName = "$", ancestors = new WeakSet<object>()): void {
  if (value === null || typeof value === "string" || typeof value === "boolean") return;
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new Error(`${pathName} must contain a finite JSON number`);
    return;
  }
  if (typeof value !== "object")
    throw new Error(`${pathName} contains non-JSON value ${typeof value}`);
  if (ancestors.has(value)) throw new Error(`${pathName} contains a circular reference`);
  const prototype = Object.getPrototypeOf(value);
  if (!Array.isArray(value) && prototype !== Object.prototype && prototype !== null)
    throw new Error(
      `${pathName} contains non-plain object ${prototype?.constructor?.name ?? "unknown"}`,
    );
  ancestors.add(value);
  if (Array.isArray(value)) {
    for (const [index, item] of value.entries())
      assertJsonSafe(item, `${pathName}[${index}]`, ancestors);
  } else {
    for (const [key, item] of Object.entries(value)) {
      if (item === undefined) continue;
      assertJsonSafe(item, `${pathName}.${key}`, ancestors);
    }
  }
  ancestors.delete(value);
}

async function exists(file: string) {
  try {
    await access(file);
    return true;
  } catch {
    return false;
  }
}
function esbuildMessage(error: unknown) {
  return error && typeof error === "object" && "errors" in error
    ? (error as { errors: Message[] }).errors.map((message) => message.text).join("\n")
    : error instanceof Error
      ? error.message
      : String(error);
}
function escapeHtml(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
