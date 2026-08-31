import { evaluateArtifact } from "../../document/artifact";
import { changedLineCounts } from "../../document/diff";
import type { SourceEvidenceResolver } from "../../document/model";
import type { NormalizedSoftwareModel } from "./model";
import {
  serializedSoftwareMapSchema,
  type CompiledSoftwareMap,
  type SerializedSoftwareMap,
} from "./schema";

export async function collectSoftwareMap(
  artifactPath: string,
  repo: string,
  modelSource: string,
): Promise<SerializedSoftwareMap[]> {
  return evaluateArtifact({
    artifactPath,
    repo,
    errorCode: "SOFTWARE_MAP_INVALID",
    moduleAliases: {
      "@dev.fast/progressive-review/software-map-model": modelSource,
    },
    read(loaded) {
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
    },
  });
}

export async function compileSoftwareMap(
  model: SerializedSoftwareMap,
  evidence: SourceEvidenceResolver,
): Promise<CompiledSoftwareMap> {
  const evidenceByPath: CompiledSoftwareMap["evidenceByPath"] = {};
  const elementsByPath = new Map(model.elements.map((element) => [element.path, element]));
  for (const element of model.elements) {
    if (!element.sourceRanges?.length) continue;
    evidenceByPath[element.path] = await Promise.all(
      element.sourceRanges.map((range) =>
        evidence.resolveRange({
          ...range,
          ...(element.changeStatus === "removed" ? { graph: "base" as const } : {}),
        }),
      ),
    );
  }
  const diffCountsByPath: CompiledSoftwareMap["diffCountsByPath"] = {};
  for (const element of model.elements) {
    const counts = changedLineCounts(element.sourceRanges ?? [], evidence);
    const additions = element.changeStatus === "removed" ? 0 : counts.additions;
    const deletions = element.changeStatus === "added" ? 0 : counts.deletions;
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
  return {
    ...model,
    evidenceByPath,
    evidenceByRelationshipId,
    diffCountsByPath,
  };
}
