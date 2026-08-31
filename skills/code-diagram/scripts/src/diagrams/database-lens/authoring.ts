import type { ReactNode } from "react";
import { z } from "zod";
import {
  actorRefSchema,
  noChildrenSchema,
  nonEmptyStringSchema,
  optionalNonEmptyStringSchema,
  peekableAnchorRefSchema,
} from "../../authoring/core";
import {
  isDataStoreFieldLeaf,
  type DataStoreFieldLeaf,
  type DataStoreFieldSchema,
} from "../../canvas/data-store-schema";
import {
  CANVAS_DATA_STORE_KINDS,
  type CanvasDataStoreKind,
} from "../../canvas/model";

export const storeKindSchema = z.enum(["relational", "document"]);
export type StoreKind = z.infer<typeof storeKindSchema>;
export const collectionKindSchema = z.enum(["tables", "documents"]);
export type CollectionKind = z.infer<typeof collectionKindSchema>;
export type SoftwareDataStoreKind = CanvasDataStoreKind;
export type SoftwareDataStoreFieldLeaf = DataStoreFieldLeaf;
export type SoftwareDataStoreFieldSchema = DataStoreFieldSchema;
export interface SoftwareDataStoreCollectionInput {
  label?: string;
  key?: string;
  schema: SoftwareDataStoreFieldSchema;
}
const fieldSchema: z.ZodType<SoftwareDataStoreFieldSchema> = z.lazy(() =>
  z.record(
    nonEmptyStringSchema,
    z.union([
      z.strictObject({
        type: nonEmptyStringSchema,
        example: z.unknown().optional(),
        pk: z.boolean().optional(),
        fk: z
          .union([
            nonEmptyStringSchema,
            z.strictObject({
              table: nonEmptyStringSchema,
              field: nonEmptyStringSchema,
              label: optionalNonEmptyStringSchema,
              cardinality: z.enum(["one-to-one", "many-to-one"]).optional(),
              onDelete: optionalNonEmptyStringSchema,
              onUpdate: optionalNonEmptyStringSchema,
            }),
          ])
          .optional(),
        schema: fieldSchema.optional(),
      }),
      fieldSchema,
    ]),
  ),
);
const collectionInputSchema = z.strictObject({
  label: optionalNonEmptyStringSchema,
  key: optionalNonEmptyStringSchema,
  schema: fieldSchema,
});
export const collectionMapSchema = z.record(
  nonEmptyStringSchema,
  collectionInputSchema,
);
export const storeInputSchema = z.strictObject({
  kind: storeKindSchema,
  label: nonEmptyStringSchema,
  dataStoreKind: z.enum(CANVAS_DATA_STORE_KINDS).optional(),
  softwareMapPath: optionalNonEmptyStringSchema,
  tables: collectionMapSchema.optional(),
  documents: collectionMapSchema.optional(),
});
export type StoreInput = z.infer<typeof storeInputSchema>;
export const storeInputMapSchema = z.record(
  nonEmptyStringSchema,
  storeInputSchema,
);
export type StoreInputMap = z.infer<typeof storeInputMapSchema>;

export interface TargetRef {
  __kind: "db-target-ref";
  storeId: string;
  storeKind: StoreKind;
  storeLabel: string;
  storeDataStoreKind?: SoftwareDataStoreKind;
  storeSoftwareMapPath?: string;
  collectionKind: CollectionKind;
  collectionId: string;
  collectionLabel: string;
  collectionKey?: string;
  path: string[];
}
const authoredTargetRefKey: unique symbol = Symbol("authored-target-ref");
const collectionSchemaKey: unique symbol = Symbol("collection-schema");
export interface AuthoredTargetRef {
  readonly [authoredTargetRefKey]: TargetRef;
}
export type CollectionHandle = AuthoredTargetRef & {
  readonly [collectionSchemaKey]: SoftwareDataStoreFieldSchema;
};
export type CollectionRef = CollectionHandle &
  Record<string, AuthoredTargetRef>;
export interface StoreRef {
  __kind: "db-store-ref";
  id: string;
  kind: StoreKind;
  label: string;
  dataStoreKind?: SoftwareDataStoreKind;
  softwareMapPath?: string;
  tables?: Record<string, CollectionRef>;
  documents?: Record<string, CollectionRef>;
}
export function resolveTargetRef(value: unknown): TargetRef | null {
  if (!value || typeof value !== "object") return null;
  const authored = (value as Partial<AuthoredTargetRef>)[authoredTargetRefKey];
  if (authored) return authored;
  return (value as { __kind?: unknown }).__kind === "db-target-ref"
    ? (value as TargetRef)
    : null;
}
export function collectionTargetRef(collection: CollectionRef): TargetRef {
  return collection[authoredTargetRefKey];
}
export function collectionSchema(
  collection: CollectionRef,
): SoftwareDataStoreFieldSchema {
  return collection[collectionSchemaKey];
}
export const targetRefSchema = z.preprocess(
  (value) => resolveTargetRef(value) ?? value,
  z.strictObject({
    __kind: z.literal("db-target-ref"),
    storeId: nonEmptyStringSchema,
    storeKind: storeKindSchema,
    storeLabel: nonEmptyStringSchema,
    storeDataStoreKind: z.enum(CANVAS_DATA_STORE_KINDS).optional(),
    storeSoftwareMapPath: optionalNonEmptyStringSchema,
    collectionKind: collectionKindSchema,
    collectionId: nonEmptyStringSchema,
    collectionLabel: nonEmptyStringSchema,
    collectionKey: optionalNonEmptyStringSchema,
    path: z.array(nonEmptyStringSchema),
  }),
);
const dbOperationCommonShape = {
  label: nonEmptyStringSchema,
  anchor: peekableAnchorRefSchema,
  children: noChildrenSchema,
};
export const dbReadPropsSchema = z.strictObject({
  from: targetRefSchema,
  to: actorRefSchema,
  ...dbOperationCommonShape,
});
export type DbReadProps = Omit<z.input<typeof dbReadPropsSchema>, "from"> & {
  from: AuthoredTargetRef;
};
export const dbWritePropsSchema = z.strictObject({
  from: actorRefSchema,
  to: targetRefSchema,
  ...dbOperationCommonShape,
});
export type DbWriteProps = Omit<z.input<typeof dbWritePropsSchema>, "to"> & {
  to: AuthoredTargetRef;
};
const reactNodeSchema = z.custom<ReactNode>();
export const dbUseCasePropsSchema = z.strictObject({
  id: nonEmptyStringSchema,
  label: nonEmptyStringSchema,
  summary: optionalNonEmptyStringSchema,
  children: reactNodeSchema,
});
export type DbUseCaseProps = z.infer<typeof dbUseCasePropsSchema>;
export const databaseLensPropsSchema = z.strictObject({
  title: optionalNonEmptyStringSchema,
  stores: z.record(
    nonEmptyStringSchema,
    z.custom<StoreRef>(
      (value) =>
        Boolean(value) &&
        typeof value === "object" &&
        (value as { __kind?: unknown }).__kind === "db-store-ref",
      "Must be a store reference returned by defineStores",
    ),
  ),
  height: z.number().positive().optional(),
  children: reactNodeSchema,
});
export type DatabaseLensProps = z.infer<typeof databaseLensPropsSchema>;

export type CollectionRefs<T> =
  T extends Record<string, SoftwareDataStoreCollectionInput>
    ? { [K in keyof T]: CollectionHandle & FieldRefs<T[K]["schema"]> }
    : never;
type FieldRefs<T> = T extends SoftwareDataStoreFieldLeaf
  ? T extends { schema: infer S extends Record<string, unknown> }
    ? AuthoredTargetRef & FieldRefs<S>
    : AuthoredTargetRef
  : T extends Record<string, unknown>
    ? {
        [K in keyof T]: AuthoredTargetRef &
          (T[K] extends SoftwareDataStoreFieldLeaf
            ? T[K] extends {
                schema: infer S extends Record<string, unknown>;
              }
              ? FieldRefs<S>
              : unknown
            : T[K] extends Record<string, unknown>
              ? FieldRefs<T[K]>
              : unknown);
      }
    : unknown;
export type StoreRefFor<T extends StoreInput> = Omit<
  StoreRef,
  "tables" | "documents"
> &
  (T["tables"] extends Record<string, SoftwareDataStoreCollectionInput>
    ? { tables: CollectionRefs<T["tables"]> }
    : { tables?: never }) &
  (T["documents"] extends Record<string, SoftwareDataStoreCollectionInput>
    ? { documents: CollectionRefs<T["documents"]> }
    : { documents?: never });

export function defineStores<T extends StoreInputMap>(
  input: T,
): { [K in keyof T]: StoreRefFor<T[K]> } {
  storeInputMapSchema.parse(input);
  return Object.fromEntries(
    Object.entries(input).map(([id, store]) => {
      const base: StoreRef = {
        __kind: "db-store-ref",
        id,
        kind: store.kind,
        label: store.label,
        dataStoreKind: store.dataStoreKind,
        softwareMapPath: store.softwareMapPath,
      };
      if (store.tables)
        base.tables = defineCollections(id, store, "tables", store.tables);
      if (store.documents)
        base.documents = defineCollections(
          id,
          store,
          "documents",
          store.documents,
        );
      return [id, Object.freeze(base)];
    }),
  ) as { [K in keyof T]: StoreRefFor<T[K]> };
}

function defineCollections(
  storeId: string,
  store: StoreInput,
  collectionKind: CollectionKind,
  collections: Record<string, SoftwareDataStoreCollectionInput>,
): Record<string, CollectionRef> {
  return Object.fromEntries(
    Object.entries(collections).map(([collectionId, collection]) => {
      const target: TargetRef = {
        __kind: "db-target-ref",
        storeId,
        storeKind: store.kind,
        storeLabel: store.label,
        storeDataStoreKind: store.dataStoreKind,
        storeSoftwareMapPath: store.softwareMapPath,
        collectionKind,
        collectionId,
        collectionLabel: collection.label ?? collectionId,
        collectionKey: collection.key,
        path: [],
      };
      const authored = defineFieldTargets(
        target,
        collection.schema,
        [],
      ) as CollectionRef & Record<PropertyKey, unknown>;
      Object.defineProperties(authored, {
        [authoredTargetRefKey]: { value: Object.freeze(target) },
        [collectionSchemaKey]: { value: collection.schema },
      });
      return [collectionId, Object.freeze(authored)];
    }),
  );
}

function defineFieldTargets(
  collection: TargetRef,
  schema: SoftwareDataStoreFieldSchema,
  prefix: string[],
): Record<string, AuthoredTargetRef> {
  return Object.fromEntries(
    Object.entries(schema).map(([field, value]) => {
      const path = [...prefix, field];
      const target = Object.freeze({ ...collection, path });
      const nested = isDataStoreFieldLeaf(value) ? value.schema : value;
      const authored = (nested
        ? defineFieldTargets(collection, nested, path)
        : {}) as AuthoredTargetRef & Record<PropertyKey, unknown>;
      Object.defineProperty(authored, authoredTargetRefKey, { value: target });
      return [field, Object.freeze(authored)];
    }),
  );
}
