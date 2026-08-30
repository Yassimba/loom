import { build, type Plugin } from "esbuild";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import type { SourceEvidenceResolver } from "./diagram-family";
import type { NormalizedSoftwareModel } from "./software-map-model";
import {
  compiledSoftwareMapSchema,
  serializedSoftwareMapSchema,
  type CompiledSoftwareMap,
  type SerializedSoftwareMap,
} from "./software-map-schema";

export async function collectSoftwareMap(
  artifactPath: string,
  repo: string,
  modelSource: string,
): Promise<SerializedSoftwareMap[]> {
  let output;
  try {
    output = await build({
      absWorkingDir: repo,
      bundle: true,
      platform: "node",
      format: "esm",
      packages: "external",
      target: ["node22"],
      write: false,
      entryPoints: [artifactPath],
      plugins: [mapAlias(modelSource)],
      logLevel: "silent",
    });
  } catch (error) {
    throw new Error(
      `SOFTWARE_MAP_INVALID: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  const code = output.outputFiles[0]?.text;
  if (!code) throw new Error("SOFTWARE_MAP_INVALID: compiler produced no output");
  const temp = await mkdtemp(path.join(tmpdir(), "code-diagram-map-"));
  const modulePath = path.join(temp, "software-map.mjs");
  try {
    await writeFile(modulePath, code);
    const loaded = await import(`${pathToFileURL(modulePath).href}?${Date.now()}`);
    const model = loaded.default as NormalizedSoftwareModel | undefined;
    if (
      !model ||
      !Array.isArray(model.elements) ||
      !(model.elementsByPath instanceof Map) ||
      !Array.isArray(model.relationships)
    )
      throw new Error("default export must be defineSoftwareMap({...})");
    return [
      serializedSoftwareMapSchema.parse({
        elements: model.elements,
        relationships: model.relationships,
      }),
    ];
  } catch (error) {
    throw new Error(
      `SOFTWARE_MAP_INVALID: ${error instanceof Error ? error.message : String(error)}`,
    );
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
}

export async function compileSoftwareMap(
  model: SerializedSoftwareMap,
  evidence: SourceEvidenceResolver,
): Promise<CompiledSoftwareMap> {
  const evidenceByPath: CompiledSoftwareMap["evidenceByPath"] = {};
  const elementsByPath = new Map(model.elements.map((element) => [element.path, element]));
  for (const element of model.elements) {
    if (!element.sourceRanges?.length) continue;
    evidenceByPath[element.path] = [];
    for (const range of element.sourceRanges)
      evidenceByPath[element.path]!.push(
        await evidence.resolveRange({
          ...range,
          ...(element.changeStatus === "removed" ? { graph: "base" as const } : {}),
        }),
      );
  }
  const diffCountsByPath: CompiledSoftwareMap["diffCountsByPath"] = {};
  for (const element of model.elements) {
    const addedLines = new Set<string>();
    const deletedLines = new Set<string>();
    for (const range of element.sourceRanges ?? []) {
      const head = evidence.changedLines(range.file, "head");
      const base = evidence.changedLines(range.file, "base");
      collectLinesInRange(addedLines, range.file, head?.added, range.fromLine, range.toLine);
      collectLinesInRange(deletedLines, range.file, base?.deleted, range.fromLine, range.toLine);
    }
    const additions = element.changeStatus === "removed" ? 0 : addedLines.size;
    const deletions = element.changeStatus === "added" ? 0 : deletedLines.size;
    if (additions || deletions) diffCountsByPath[element.path] = { additions, deletions };
  }
  const evidenceByRelationshipId: CompiledSoftwareMap["evidenceByRelationshipId"] = {};
  for (const relationship of model.relationships) {
    const sourceElement = elementsByPath.get(relationship.scopePath ?? relationship.from);
    const graph = sourceElement?.changeStatus === "removed" ? ("base" as const) : undefined;
    const file = sourceElement?.sourceRanges?.[0]?.file;
    if (relationship.kind === "semantic" && file && relationship.sourceRanges?.length) {
      evidenceByRelationshipId[relationship.id] = await Promise.all(
        relationship.sourceRanges.map((range) => evidence.resolveRange({ file, ...range, graph })),
      );
      continue;
    }
    const fallback = evidenceByPath[relationship.from];
    if (fallback?.length) evidenceByRelationshipId[relationship.id] = fallback;
  }
  return compiledSoftwareMapSchema.parse({
    ...model,
    evidenceByPath,
    evidenceByRelationshipId,
    diffCountsByPath,
  });
}

function collectLinesInRange(
  result: Set<string>,
  file: string,
  lines: ReadonlySet<number> | undefined,
  fromLine: number,
  toLine: number,
) {
  if (!lines) return;
  for (const line of lines) if (line >= fromLine && line <= toLine) result.add(`${file}:${line}`);
}

function mapAlias(modelSource: string): Plugin {
  return {
    name: "software-map-alias",
    setup(pluginBuild) {
      pluginBuild.onResolve(
        { filter: /^@dev\.fast\/progressive-review\/software-map-model$/ },
        () => ({ path: modelSource }),
      );
    },
  };
}
