import type { ReviewSurfaceDescriptor } from "../../document/model";
import {
  capturedDatabaseLensSchema,
  compiledDatabaseLensSchema,
  compileDatabaseLens,
  createDatabaseLensComponents,
  type CapturedDatabaseLens,
  type CompiledDatabaseLens,
} from "./model";

export const databaseLensSurfaceDescriptor = {
  kind: "database-lens",
  source: { type: "mdx", createComponents: createDatabaseLensComponents },
  capturedSchema: capturedDatabaseLensSchema,
  compile: compileDatabaseLens,
  compiledSchema: compiledDatabaseLensSchema,
  browser: {
    specifier: "./src/diagrams/database-lens/canvas.tsx",
    assets: ["libavoid"],
  },
} satisfies ReviewSurfaceDescriptor<
  "database-lens",
  CapturedDatabaseLens,
  CompiledDatabaseLens
>;
