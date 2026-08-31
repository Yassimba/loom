import type { CompiledDatabaseLens } from "../database-lens/model";
import type { SequenceDiagramProps } from "../sequence/authoring";
import type { CompiledSurface } from "../../document/model";

export function validateSoftwareMapReferences(
  diagrams: readonly CompiledSurface[],
) {
  const map = diagrams.find((diagram) => diagram.kind === "software-map")
    ?.model as { elements?: Array<{ path?: unknown }> } | undefined;
  const knownPaths = new Set(
    map?.elements?.flatMap((element) =>
      typeof element.path === "string" ? [element.path] : [],
    ) ?? [],
  );
  const references = new Set<string>();
  for (const diagram of diagrams) {
    if (diagram.kind === "sequence") {
      for (const message of (diagram.model as SequenceDiagramProps).messages) {
        addSoftwareMapPath(references, message.from);
        addSoftwareMapPath(references, message.to);
      }
    }
    if (diagram.kind === "database-lens") {
      const model = diagram.model as CompiledDatabaseLens;
      for (const store of model.stores) {
        if (store.softwareMapPath) references.add(store.softwareMapPath);
      }
      for (const operation of model.useCases.flatMap(
        (useCase) => useCase.operations,
      )) {
        addSoftwareMapPath(references, operation.actor);
      }
    }
  }
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

function addSoftwareMapPath(
  references: Set<string>,
  value: unknown,
) {
  if (
    value &&
    typeof value === "object" &&
    "softwareMapPath" in value &&
    typeof value.softwareMapPath === "string"
  ) {
    references.add(value.softwareMapPath);
  }
}
