import { z } from "zod";
import { CANVAS_DATA_STORE_KINDS } from "../../canvas/model";
import { CANVAS_RELATIONSHIP_SEMANTIC_KINDS } from "../../canvas/semantics";
import { changeStateSchema, sourceRangeSchema } from "../../document/model";

const jsonValueSchema = z.preprocess((value) => removeUndefinedProperties(value), z.json());

const elementSchema = z.strictObject({
  type: z.enum(["person", "softwareSystem", "container", "dataStore", "component", "codeElement"]),
  id: z.string().min(1),
  path: z.string().min(1),
  parentPath: z.string().min(1).optional(),
  label: z.string().min(1),
  description: z.string().optional(),
  changeStatus: changeStateSchema.optional(),
  coverage: jsonValueSchema.optional(),
  external: z.boolean().optional(),
  dataStoreKind: z.enum(CANVAS_DATA_STORE_KINDS).optional(),
  dataStoreSchema: jsonValueSchema.optional(),
  sourceRanges: z
    .array(
      z.strictObject({
        file: z.string().min(1),
        fromLine: z.int().positive(),
        toLine: z.int().positive(),
      }),
    )
    .optional(),
  children: z.array(z.string().min(1)),
});
const relationshipSchema = z.union([
  z.strictObject({
    id: z.string(),
    from: z.string(),
    to: z.string(),
    scopePath: z.string().optional(),
    label: z.string().optional(),
    description: z.string().optional(),
    kind: z.literal("call"),
    nthCallSite: z.number().int().nonnegative(),
  }),
  z.strictObject({
    id: z.string(),
    from: z.string(),
    to: z.string(),
    scopePath: z.string().optional(),
    label: z.string().optional(),
    description: z.string().optional(),
    kind: z.literal("semantic"),
    semanticKind: z.enum(CANVAS_RELATIONSHIP_SEMANTIC_KINDS).optional(),
    sourceRanges: z
      .array(z.strictObject({ fromLine: z.int().positive(), toLine: z.int().positive() }))
      .optional(),
  }),
]);
export const serializedSoftwareMapSchema = z.strictObject({
  elements: z.array(elementSchema),
  relationships: z.array(relationshipSchema),
});
export type SerializedSoftwareMap = z.infer<typeof serializedSoftwareMapSchema>;
const diffCountsSchema = z.strictObject({
  additions: z.int().nonnegative(),
  deletions: z.int().nonnegative(),
});
export const compiledSoftwareMapSchema = serializedSoftwareMapSchema.extend({
  evidenceByPath: z.record(z.string(), z.array(sourceRangeSchema)),
  evidenceByRelationshipId: z.record(z.string(), z.array(sourceRangeSchema)),
  diffCountsByPath: z.record(z.string(), diffCountsSchema),
});
export type CompiledSoftwareMap = z.infer<typeof compiledSoftwareMapSchema>;

function removeUndefinedProperties(value: unknown, seen = new WeakMap<object, unknown>()): unknown {
  if (!value || typeof value !== "object") return value;
  const cached = seen.get(value);
  if (cached) return cached;
  if (Array.isArray(value)) {
    const result: unknown[] = [];
    seen.set(value, result);
    for (const item of value) result.push(removeUndefinedProperties(item, seen));
    return result;
  }
  const result: Record<string, unknown> = {};
  seen.set(value, result);
  for (const [key, item] of Object.entries(value))
    if (item !== undefined) result[key] = removeUndefinedProperties(item, seen);
  return result;
}
