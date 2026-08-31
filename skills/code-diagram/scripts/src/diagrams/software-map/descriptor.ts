import type { ReviewSurfaceDescriptor } from "../../document/model";
import { collectSoftwareMap, compileSoftwareMap } from "./compile";
import { validateSoftwareMapReferences } from "./validate";
import {
  compiledSoftwareMapSchema,
  serializedSoftwareMapSchema,
  type CompiledSoftwareMap,
  type SerializedSoftwareMap,
} from "./schema";

export function createSoftwareMapSurfaceDescriptor(modelSource: string) {
  return {
    kind: "software-map",
    source: {
      type: "artifact",
      fileName: "software-map.ts",
      typecheck: {
        moduleAliases: {
          "@dev.fast/progressive-review/software-map-model": modelSource,
        },
      },
      collect: (artifactPath, repo) =>
        collectSoftwareMap(artifactPath, repo, modelSource),
    },
    capturedSchema: serializedSoftwareMapSchema,
    compile: compileSoftwareMap,
    compiledSchema: compiledSoftwareMapSchema,
    validateDocument: validateSoftwareMapReferences,
    browser: {
      specifier: "./src/diagrams/software-map/viewer.tsx",
      assets: ["libavoid"],
    },
  } satisfies ReviewSurfaceDescriptor<
    "software-map",
    SerializedSoftwareMap,
    CompiledSoftwareMap
  >;
}
