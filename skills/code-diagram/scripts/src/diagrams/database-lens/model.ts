import React, { type ReactElement, type ReactNode } from "react";
import { z } from "zod";

import { actorRefSchema } from "../../authoring/core";
import { CANVAS_DATA_STORE_KINDS } from "../../canvas/model";
import {
  collectionSchema,
  collectionTargetRef,
  databaseLensPropsSchema,
  dbReadPropsSchema,
  dbUseCasePropsSchema,
  dbWritePropsSchema,
  targetRefSchema,
  type DatabaseLensProps,
  type DbReadProps,
  type DbUseCaseProps,
  type DbWriteProps,
  type SoftwareDataStoreFieldSchema,
  type StoreRef,
} from "./authoring";
import { resolveAnchorEvidence } from "../../document/anchor-evidence";
import { sourceRangeSchema, type SourceEvidenceResolver } from "../../document/model";

const serializedForeignKeySchema = z.union([
  z.string().min(1),
  z.strictObject({
    table: z.string().min(1),
    field: z.string().min(1),
    label: z.string().min(1).optional(),
    cardinality: z.enum(["one-to-one", "many-to-one"]).optional(),
    onDelete: z.string().min(1).optional(),
    onUpdate: z.string().min(1).optional(),
  }),
]);
const serializedFieldSchema: z.ZodType<SoftwareDataStoreFieldSchema> = z.lazy(() =>
  z.record(
    z.string().min(1),
    z.union([
      z.strictObject({
        type: z.string().min(1),
        example: z.json().optional(),
        pk: z.boolean().optional(),
        fk: serializedForeignKeySchema.optional(),
        schema: serializedFieldSchema.optional(),
      }),
      serializedFieldSchema,
    ]),
  ),
);
const collectionSchemaPlain = z.strictObject({
  id: z.string(),
  kind: z.enum(["tables", "documents"]),
  label: z.string(),
  key: z.string().optional(),
  schema: serializedFieldSchema,
});
const storeSchema = z.strictObject({
  id: z.string(),
  kind: z.enum(["relational", "document"]),
  label: z.string(),
  dataStoreKind: z.enum(CANVAS_DATA_STORE_KINDS).optional(),
  softwareMapPath: z.string().optional(),
  collections: z.array(collectionSchemaPlain),
});
const operationSchema = z.strictObject({
  kind: z.enum(["read", "write"]),
  label: z.string(),
  actor: actorRefSchema,
  target: targetRefSchema,
  anchor: z.any(),
});
const useCaseSchema = z.strictObject({
  id: z.string(),
  label: z.string(),
  summary: z.string().optional(),
  operations: z.array(operationSchema).min(1),
});
export const capturedDatabaseLensSchema = z.strictObject({
  title: z.string().optional(),
  height: z.number().positive().optional(),
  stores: z.array(storeSchema).min(1),
  useCases: z.array(useCaseSchema).min(1),
});
export type CapturedDatabaseLens = z.infer<typeof capturedDatabaseLensSchema>;
export const compiledDatabaseLensSchema = capturedDatabaseLensSchema.extend({
  useCases: z.array(
    useCaseSchema.extend({
      operations: z.array(operationSchema.extend({ source: sourceRangeSchema })),
    }),
  ),
});
export type CompiledDatabaseLens = z.infer<typeof compiledDatabaseLensSchema>;

export function createDatabaseLensComponents(
  emit: (model: CapturedDatabaseLens) => ReactElement,
): Record<string, React.ComponentType<any>> {
  function DbUseCase(_props: DbUseCaseProps): ReactNode {
    throw new Error("DbUseCase must be a direct child of DatabaseLens");
  }
  function DbRead(_props: DbReadProps): ReactNode {
    throw new Error("DbRead must be a direct child of DbUseCase");
  }
  function DbWrite(_props: DbWriteProps): ReactNode {
    throw new Error("DbWrite must be a direct child of DbUseCase");
  }
  function DatabaseLens(raw: DatabaseLensProps): ReactElement {
    const props = databaseLensPropsSchema.parse(raw);
    const useCases = React.Children.toArray(props.children).flatMap<
      z.infer<typeof useCaseSchema>
    >((child) => {
      if (!React.isValidElement(child) || child.type !== DbUseCase) return [];
      const useCase = dbUseCasePropsSchema.parse(child.props);
      const operations = React.Children.toArray(useCase.children).flatMap<
        z.infer<typeof operationSchema>
      >((operation) => {
        if (
          !React.isValidElement(operation) ||
          (operation.type !== DbRead && operation.type !== DbWrite)
        )
          return [];
        if (operation.type === DbRead) {
          const parsed = dbReadPropsSchema.parse(operation.props);
          return [
            {
              kind: "read" as const,
              label: parsed.label,
              actor: parsed.to,
              target: parsed.from,
              anchor: parsed.anchor,
            },
          ];
        }
        const parsed = dbWritePropsSchema.parse(operation.props);
        return [
          {
            kind: "write" as const,
            label: parsed.label,
            actor: parsed.from,
            target: parsed.to,
            anchor: parsed.anchor,
          },
        ];
      });
      if (operations.length === 0)
        throw new Error(`DbUseCase "${useCase.label}" must contain at least one operation`);
      return [{ id: useCase.id, label: useCase.label, summary: useCase.summary, operations }];
    });
    if (useCases.length === 0) throw new Error("DatabaseLens must contain at least one DbUseCase");
    const labels = new Set<string>();
    for (const useCase of useCases) {
      if (labels.has(useCase.label))
        throw new Error(`DatabaseLens use-case label "${useCase.label}" must be unique`);
      labels.add(useCase.label);
    }
    const stores = Object.values(props.stores).map(plainStore);
    return emit(
      capturedDatabaseLensSchema.parse({
        title: props.title,
        height: props.height,
        stores,
        useCases,
      }),
    );
  }
  return { DatabaseLens, DbUseCase, DbRead, DbWrite };
}

function plainStore(store: StoreRef) {
  const collections = (["tables", "documents"] as const).flatMap((kind) =>
    Object.entries(store[kind] ?? {}).map(([id, collection]) => {
      const target = collectionTargetRef(collection);
      return {
        id,
        kind,
        label: target.collectionLabel,
        key: target.collectionKey,
        schema: collectionSchema(collection),
      };
    }),
  );
  return {
    id: store.id,
    kind: store.kind,
    label: store.label,
    dataStoreKind: store.dataStoreKind,
    softwareMapPath: store.softwareMapPath,
    collections,
  };
}

export async function compileDatabaseLens(
  model: CapturedDatabaseLens,
  evidence: SourceEvidenceResolver,
): Promise<CompiledDatabaseLens> {
  return {
    ...model,
    useCases: await Promise.all(
      model.useCases.map(async (useCase) => ({
        ...useCase,
        operations: await Promise.all(
          useCase.operations.map(async (operation) => {
            const resolved = await resolveAnchorEvidence(operation.anchor, evidence);
            return { ...operation, anchor: resolved.anchor, source: resolved.source };
          }),
        ),
      })),
    ),
  };
}
