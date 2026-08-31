import { z } from "zod";
import type {
  NormalizedSoftwareModel,
  SoftwareDataStoreKind as ModelDataStoreKind,
} from "../diagrams/software-map/model";
import {
  nonEmptyStringSchema,
  optionalNonEmptyStringSchema,
  throwAuthoringIssue,
  type ActorRef,
} from "./core";
import {
  collectionMapSchema,
  defineStores,
  storeKindSchema,
  type CollectionRefs,
  type SoftwareDataStoreCollectionInput,
  type StoreInputMap,
  type StoreKind,
  type StoreRef,
} from "../diagrams/database-lens/authoring";

export const softwareActorInputSchema = z.union([
  nonEmptyStringSchema,
  z.strictObject({
    path: nonEmptyStringSchema,
    label: optionalNonEmptyStringSchema,
  }),
]);
export type SoftwareActorInput = z.infer<typeof softwareActorInputSchema>;
export const softwareStoreInputSchema = z.strictObject({
  path: nonEmptyStringSchema,
  label: optionalNonEmptyStringSchema,
  kind: storeKindSchema.optional(),
  tables: collectionMapSchema.optional(),
  documents: collectionMapSchema.optional(),
});
export type SoftwareStoreInput = z.infer<typeof softwareStoreInputSchema>;
export const softwareStoreInputMapSchema = z.record(
  nonEmptyStringSchema,
  softwareStoreInputSchema,
);
export type SoftwareStoreInputMap = z.infer<
  typeof softwareStoreInputMapSchema
>;

type SoftwareStoreRefFor<T extends SoftwareStoreInput> = Omit<
  StoreRef,
  "tables" | "documents"
> &
  (T["tables"] extends Record<string, SoftwareDataStoreCollectionInput>
    ? { tables: CollectionRefs<T["tables"]> }
    : Pick<StoreRef, "tables">) &
  (T["documents"] extends Record<string, SoftwareDataStoreCollectionInput>
    ? { documents: CollectionRefs<T["documents"]> }
    : Pick<StoreRef, "documents">);

export function defineSoftwareActors<T extends Record<string, SoftwareActorInput>>(
  model: NormalizedSoftwareModel,
  input: T,
): { [K in keyof T]: ActorRef } {
  z.record(nonEmptyStringSchema, softwareActorInputSchema).parse(input);
  return Object.fromEntries(
    Object.entries(input).map(([id, actor]) => {
      const path = typeof actor === "string" ? actor : actor.path;
      const element = softwareElementForPath(model, path, [id, "path"]);
      return [
        id,
        Object.freeze({
          __kind: "db-actor-ref",
          id,
          label:
            typeof actor === "string"
              ? element.label
              : (actor.label ?? element.label),
          softwareMapPath: element.path,
        } satisfies ActorRef),
      ];
    }),
  ) as { [K in keyof T]: ActorRef };
}

export function defineSoftwareStores<T extends SoftwareStoreInputMap>(
  model: NormalizedSoftwareModel,
  input: T,
): { [K in keyof T]: SoftwareStoreRefFor<T[K]> } {
  softwareStoreInputMapSchema.parse(input);
  const stores = Object.fromEntries(
    Object.entries(input).map(([id, store]) => {
      const element = softwareElementForPath(model, store.path, [id, "path"]);
      if (element.type !== "dataStore")
        throwAuthoringIssue(
          [id, "path"],
          `Software map element "${store.path}" must be a dataStore to back a DatabaseLens store`,
        );
      return [
        id,
        {
          kind:
            store.kind ?? DATA_STORE_KIND_MAP[element.dataStoreKind ?? "database"],
          label: store.label ?? element.label,
          dataStoreKind: element.dataStoreKind,
          softwareMapPath: element.path,
          tables: store.tables ?? authoredCollections(element.dataStoreSchema?.tables),
          documents:
            store.documents ??
            authoredCollections(element.dataStoreSchema?.documents),
        },
      ];
    }),
  ) as StoreInputMap;
  return defineStores(stores) as {
    [K in keyof T]: SoftwareStoreRefFor<T[K]>;
  };
}

function authoredCollections(
  collections:
    | Record<string, SoftwareDataStoreCollectionInput & { id?: string }>
    | undefined,
): Record<string, SoftwareDataStoreCollectionInput> | undefined {
  if (!collections) return undefined;
  return Object.fromEntries(
    Object.entries(collections).map(
      ([key, { id: _id, ...collection }]) => [key, collection],
    ),
  );
}

function softwareElementForPath(
  model: NormalizedSoftwareModel,
  path: string,
  propertyPath: PropertyKey[],
) {
  const element = model.elementsByPath.get(path);
  if (!element)
    throwAuthoringIssue(
      propertyPath,
      "Must reference an existing software-map path",
    );
  return element;
}

const DATA_STORE_KIND_MAP: Record<ModelDataStoreKind, StoreKind> = {
  artifactStore: "document",
  bucket: "document",
  database: "relational",
  fileStore: "document",
  objectStore: "document",
};
