// Ported from devdotfast/review@8267620:
// packages/progressive-review/src/authoring.ts.
import type { ReactNode } from "react";
import { z } from "zod";
import type {
  NormalizedSoftwareModel,
  SoftwareDataStoreKind as ModelDataStoreKind,
} from "./software-map-model";

const nonEmptyStringSchema = z
  .string()
  .refine((value) => value.trim().length > 0, "Must not be empty");
const optionalNonEmptyStringSchema = nonEmptyStringSchema.optional();
const noChildrenSchema = z.never().optional();

export interface CodePeekResolutionContext {
  anchorId: string;
}

export interface SourceLine {
  number: number;
  text: string;
}

export interface CodePeekResolution {
  source: {
    file: string;
    fromLine: number;
    toLine: number;
    lines: SourceLine[];
  };
}

export interface ReviewDefinitionEnvironment {
  resolveCodePeek?(
    props: CodePeekProps,
    context?: CodePeekResolutionContext,
  ): Promise<CodePeekResolution>;
}

export const actorInputSchema = z.strictObject({
  label: nonEmptyStringSchema,
  softwareMapPath: optionalNonEmptyStringSchema,
});
export type ActorInput = z.infer<typeof actorInputSchema>;

export const actorInputMapSchema = z.record(nonEmptyStringSchema, actorInputSchema);
export type ActorInputMap = z.infer<typeof actorInputMapSchema>;

export const actorRefSchema = z.strictObject({
  __kind: z.literal("db-actor-ref"),
  id: nonEmptyStringSchema,
  label: nonEmptyStringSchema,
  softwareMapPath: optionalNonEmptyStringSchema,
});
export type ActorRef = z.infer<typeof actorRefSchema>;

const inlineSequenceActorSchema = z.strictObject({
  id: optionalNonEmptyStringSchema,
  label: nonEmptyStringSchema,
});
export const sequenceActorInputSchema = z.union([actorRefSchema, inlineSequenceActorSchema]);
export type SequenceActorInput = z.infer<typeof sequenceActorInputSchema>;

export const sequenceMessageCodeInputSchema = z.union([
  nonEmptyStringSchema,
  z.strictObject({
    language: optionalNonEmptyStringSchema,
    text: nonEmptyStringSchema,
  }),
]);
export type SequenceMessageCodeInput = z.infer<typeof sequenceMessageCodeInputSchema>;

const codePeekCommonShape = {
  theme: z.enum(["system", "light", "dark"]).optional(),
  graph: z.enum(["head", "base"]).optional(),
  children: noChildrenSchema,
};

export const codePeekRangeInputSchema = z
  .strictObject({
    file: nonEmptyStringSchema,
    fromLine: z.int().positive(),
    toLine: z.int().positive(),
    ...codePeekCommonShape,
  })
  .refine((value) => value.toLine >= value.fromLine, {
    path: ["toLine"],
    message: "Must be greater than or equal to fromLine",
  });
export type CodePeekProps = z.infer<typeof codePeekRangeInputSchema>;

export const codePeekRefSchema = z.strictObject({
  __kind: z.literal("code-peek-ref"),
  props: codePeekRangeInputSchema,
  resolution: z.custom<CodePeekResolution>().nullable(),
});
export type CodePeekRef = z.infer<typeof codePeekRefSchema>;

export const anchorInputSchema = z.strictObject({
  title: nonEmptyStringSchema,
  peek: codePeekRangeInputSchema.optional(),
  detail: optionalNonEmptyStringSchema,
  softwareMapPath: optionalNonEmptyStringSchema,
});
export type AnchorInput = z.infer<typeof anchorInputSchema>;

export const anchorInputMapSchema = z.record(
  nonEmptyStringSchema,
  z.union([nonEmptyStringSchema, anchorInputSchema]),
);
export type AnchorInputMap = z.infer<typeof anchorInputMapSchema>;

export const anchorRefSchema = z.strictObject({
  __kind: z.literal("db-anchor-ref"),
  id: nonEmptyStringSchema,
  title: nonEmptyStringSchema,
  detail: optionalNonEmptyStringSchema,
  peek: codePeekRefSchema.optional(),
  softwareMapPath: optionalNonEmptyStringSchema,
});
export type AnchorRef = z.infer<typeof anchorRefSchema>;

export const peekableAnchorRefSchema = anchorRefSchema.extend({
  peek: codePeekRefSchema,
});
export type PeekableAnchorRef = z.infer<typeof peekableAnchorRefSchema>;

export type AnchorRefFor<T extends AnchorInputMap[string]> = AnchorRef &
  (T extends { peek: infer Peek extends CodePeekProps }
    ? { peek: CodePeekRef & { props: Peek } }
    : unknown);

const sequenceMessageBaseShape = {
  from: sequenceActorInputSchema,
  to: sequenceActorInputSchema,
  label: nonEmptyStringSchema,
};
export const sequenceMessageInputSchema = z.union([
  z.strictObject({
    ...sequenceMessageBaseShape,
    anchor: peekableAnchorRefSchema,
    code: sequenceMessageCodeInputSchema.optional(),
  }),
  z.strictObject({
    ...sequenceMessageBaseShape,
    anchor: anchorRefSchema.optional(),
    code: sequenceMessageCodeInputSchema,
  }),
]);
export type SequenceMessageInput = z.infer<typeof sequenceMessageInputSchema>;

export const sequenceDiagramPropsSchema = z.strictObject({
  label: nonEmptyStringSchema,
  messages: z.array(sequenceMessageInputSchema).min(1),
  children: noChildrenSchema,
});
export type SequenceDiagramProps = z.infer<typeof sequenceDiagramPropsSchema>;

const reactNodeSchema = z.custom<ReactNode>();

export const callsAssertionSchema = z.strictObject({
  __kind: z.literal("call-assertion"),
  parent: peekableAnchorRefSchema,
  child: peekableAnchorRefSchema,
  reason: optionalNonEmptyStringSchema,
});
export type CallsAssertion = z.infer<typeof callsAssertionSchema>;

export function calls(
  parent: PeekableAnchorRef,
  child: PeekableAnchorRef,
  reason?: string,
): CallsAssertion {
  peekableAnchorRefSchema.parse(parent);
  peekableAnchorRefSchema.parse(child);
  if (reason !== undefined) nonEmptyStringSchema.parse(reason);
  return Object.freeze({
    __kind: "call-assertion",
    parent,
    child,
    ...(reason === undefined ? {} : { reason }),
  });
}

export const callStackEntrySchema = z.union([peekableAnchorRefSchema, callsAssertionSchema]);
export type CallStackEntry = z.infer<typeof callStackEntrySchema>;
export function isCallsAssertion(value: unknown): value is CallsAssertion {
  return (
    Boolean(value) &&
    typeof value === "object" &&
    (value as { __kind?: unknown }).__kind === "call-assertion"
  );
}
export function callStackEntryAnchor(entry: CallStackEntry): PeekableAnchorRef {
  return isCallsAssertion(entry) ? entry.child : entry;
}
export const callStackDiffPropsSchema = z
  .strictObject({
    title: optionalNonEmptyStringSchema,
    base: z.array(callStackEntrySchema),
    head: z.array(callStackEntrySchema),
    children: noChildrenSchema,
  })
  .superRefine((value, context) => {
    if (value.base.length === 0 && value.head.length === 0) {
      context.addIssue({
        code: "custom",
        path: ["head"],
        message: "Must list at least one frame on base or head",
      });
      return;
    }
    const headIds = new Set(value.head.map((entry) => callStackEntryAnchor(entry).id));
    value.head.forEach((entry, index) => {
      const anchor = callStackEntryAnchor(entry);
      if (anchor.peek.props.graph === "base")
        context.addIssue({
          code: "custom",
          path: ["head", index],
          message: `Anchor "${anchor.id}" points at base; a head frame must point at head`,
        });
    });
    value.base.forEach((entry, index) => {
      const anchor = callStackEntryAnchor(entry);
      if (anchor.peek.props.graph !== "base" && !headIds.has(anchor.id))
        context.addIssue({
          code: "custom",
          path: ["base", index],
          message: `Anchor "${anchor.id}" is a removed frame; give it graph: "base" so it points at the old code`,
        });
    });
  });
export type CallStackDiffProps = z.infer<typeof callStackDiffPropsSchema>;

export const storeKindSchema = z.enum(["relational", "document"]);
export type StoreKind = z.infer<typeof storeKindSchema>;
export const collectionKindSchema = z.enum(["tables", "documents"]);
export type CollectionKind = z.infer<typeof collectionKindSchema>;
export type SoftwareDataStoreKind =
  | "database"
  | "objectStore"
  | "bucket"
  | "artifactStore"
  | "fileStore";
export interface SoftwareDataStoreFieldLeaf {
  type: string;
  example?: unknown;
  pk?: boolean;
  fk?:
    | string
    | {
        table: string;
        field: string;
        label?: string;
        cardinality?: "one-to-one" | "many-to-one";
        onDelete?: string;
        onUpdate?: string;
      };
  schema?: SoftwareDataStoreFieldSchema;
}
export type SoftwareDataStoreFieldSchema = {
  [field: string]: SoftwareDataStoreFieldLeaf | SoftwareDataStoreFieldSchema;
};
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
const collectionMapSchema = z.record(nonEmptyStringSchema, collectionInputSchema);
export const storeInputSchema = z.strictObject({
  kind: storeKindSchema,
  label: nonEmptyStringSchema,
  dataStoreKind: z
    .enum(["database", "objectStore", "bucket", "artifactStore", "fileStore"])
    .optional(),
  softwareMapPath: optionalNonEmptyStringSchema,
  tables: collectionMapSchema.optional(),
  documents: collectionMapSchema.optional(),
});
export type StoreInput = z.infer<typeof storeInputSchema>;
export const storeInputMapSchema = z.record(nonEmptyStringSchema, storeInputSchema);
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
type CollectionHandle = AuthoredTargetRef & {
  readonly [collectionSchemaKey]: SoftwareDataStoreFieldSchema;
};
export type CollectionRef = CollectionHandle & Record<string, AuthoredTargetRef>;
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
  return (value as { __kind?: unknown }).__kind === "db-target-ref" ? (value as TargetRef) : null;
}
export function collectionTargetRef(collection: CollectionRef): TargetRef {
  return collection[authoredTargetRefKey];
}
export function collectionSchema(collection: CollectionRef): SoftwareDataStoreFieldSchema {
  return collection[collectionSchemaKey];
}
const targetRefSchema = z.preprocess(
  (value) => resolveTargetRef(value) ?? value,
  z.strictObject({
    __kind: z.literal("db-target-ref"),
    storeId: nonEmptyStringSchema,
    storeKind: storeKindSchema,
    storeLabel: nonEmptyStringSchema,
    storeDataStoreKind: z
      .enum(["database", "objectStore", "bucket", "artifactStore", "fileStore"])
      .optional(),
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

type CollectionRefs<T> =
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
            ? T[K] extends { schema: infer S extends Record<string, unknown> }
              ? FieldRefs<S>
              : unknown
            : T[K] extends Record<string, unknown>
              ? FieldRefs<T[K]>
              : unknown);
      }
    : unknown;
export type StoreRefFor<T extends StoreInput> = Omit<StoreRef, "tables" | "documents"> &
  (T["tables"] extends Record<string, SoftwareDataStoreCollectionInput>
    ? { tables: CollectionRefs<T["tables"]> }
    : { tables?: never }) &
  (T["documents"] extends Record<string, SoftwareDataStoreCollectionInput>
    ? { documents: CollectionRefs<T["documents"]> }
    : { documents?: never });

type SoftwareStoreRefFor<T extends SoftwareStoreInput> = Omit<StoreRef, "tables" | "documents"> &
  (T["tables"] extends Record<string, SoftwareDataStoreCollectionInput>
    ? { tables: CollectionRefs<T["tables"]> }
    : Pick<StoreRef, "tables">) &
  (T["documents"] extends Record<string, SoftwareDataStoreCollectionInput>
    ? { documents: CollectionRefs<T["documents"]> }
    : Pick<StoreRef, "documents">);

export const softwareActorInputSchema = z.union([
  nonEmptyStringSchema,
  z.strictObject({ path: nonEmptyStringSchema, label: optionalNonEmptyStringSchema }),
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
export const softwareStoreInputMapSchema = z.record(nonEmptyStringSchema, softwareStoreInputSchema);
export type SoftwareStoreInputMap = z.infer<typeof softwareStoreInputMapSchema>;

export interface ReviewDefinitionSession {
  begin(): void;
  ready(): Promise<void>;
  defineActors<T extends ActorInputMap>(input: T): { [K in keyof T]: ActorRef };
  defineAnchors<T extends AnchorInputMap>(input: T): { [K in keyof T]: AnchorRefFor<T[K]> };
  defineStores<T extends StoreInputMap>(input: T): { [K in keyof T]: StoreRefFor<T[K]> };
  defineSoftwareActors<T extends Record<string, SoftwareActorInput>>(
    model: NormalizedSoftwareModel,
    input: T,
  ): { [K in keyof T]: ActorRef };
  defineSoftwareStores<T extends SoftwareStoreInputMap>(
    model: NormalizedSoftwareModel,
    input: T,
  ): { [K in keyof T]: SoftwareStoreRefFor<T[K]> };
}

export function createReviewDefinitionSession(
  environment: ReviewDefinitionEnvironment,
): ReviewDefinitionSession {
  let pending: Promise<void>[] = [];
  return {
    begin() {
      pending = [];
    },
    async ready() {
      await Promise.all(pending);
    },
    defineActors: (input) => defineActors(input),
    defineAnchors: (input) => defineAnchors(input, environment, pending),
    defineStores: (input) => defineStores(input),
    defineSoftwareActors,
    defineSoftwareStores,
  };
}

function defineActors<T extends ActorInputMap>(input: T): { [K in keyof T]: ActorRef } {
  actorInputMapSchema.parse(input);
  return Object.fromEntries(
    Object.entries(input).map(([id, actor]) => [
      id,
      Object.freeze({
        __kind: "db-actor-ref",
        id,
        label: actor.label,
        softwareMapPath: actor.softwareMapPath,
      } satisfies ActorRef),
    ]),
  ) as { [K in keyof T]: ActorRef };
}

function defineAnchors<T extends AnchorInputMap>(
  input: T,
  environment: ReviewDefinitionEnvironment,
  pending: Promise<void>[],
): { [K in keyof T]: AnchorRefFor<T[K]> } {
  anchorInputMapSchema.parse(input);
  return Object.fromEntries(
    Object.entries(input).map(([id, rawAnchor]) => {
      const anchor =
        typeof rawAnchor === "string" ? { title: rawAnchor } : (rawAnchor as AnchorInput);
      let peek: CodePeekRef | undefined;
      if (anchor.peek) {
        peek = {
          __kind: "code-peek-ref",
          props: codePeekRangeInputSchema.parse(anchor.peek),
          resolution: null,
        };
        const resolveCodePeek = environment.resolveCodePeek;
        if (resolveCodePeek) {
          const resolution = resolveCodePeek(anchor.peek, { anchorId: id }).then(
            (resolved) => {
              peek!.resolution = resolved;
              Object.freeze(peek);
            },
            (error: unknown) => {
              throwAuthoringIssue(
                [id, "peek"],
                `Code range could not be resolved in the pinned worktree: ${errorMessage(error)}`,
              );
            },
          );
          void resolution.catch(() => undefined);
          pending.push(resolution);
        }
      }
      return [
        id,
        Object.freeze({
          __kind: "db-anchor-ref",
          id,
          ...anchor,
          peek,
        } satisfies AnchorRef),
      ];
    }),
  ) as { [K in keyof T]: AnchorRefFor<T[K]> };
}

function defineStores<T extends StoreInputMap>(input: T): { [K in keyof T]: StoreRefFor<T[K]> } {
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
      if (store.tables) base.tables = defineCollections(id, store, "tables", store.tables);
      if (store.documents)
        base.documents = defineCollections(id, store, "documents", store.documents);
      return [id, Object.freeze(base)];
    }),
  ) as { [K in keyof T]: StoreRefFor<T[K]> };
}

function defineSoftwareActors<T extends Record<string, SoftwareActorInput>>(
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
          label: typeof actor === "string" ? element.label : (actor.label ?? element.label),
          softwareMapPath: element.path,
        } satisfies ActorRef),
      ];
    }),
  ) as { [K in keyof T]: ActorRef };
}

function defineSoftwareStores<T extends SoftwareStoreInputMap>(
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
          kind: store.kind ?? storeKindForDataStore(element.dataStoreKind),
          label: store.label ?? element.label,
          dataStoreKind: element.dataStoreKind,
          softwareMapPath: element.path,
          tables: store.tables ?? authoredCollections(element.dataStoreSchema?.tables),
          documents: store.documents ?? authoredCollections(element.dataStoreSchema?.documents),
        },
      ];
    }),
  ) as StoreInputMap;
  return defineStores(stores) as { [K in keyof T]: SoftwareStoreRefFor<T[K]> };
}

function authoredCollections(
  collections:
    | Record<string, SoftwareDataStoreCollectionInput & { id?: string }>
    | undefined,
): Record<string, SoftwareDataStoreCollectionInput> | undefined {
  if (!collections) return undefined;
  return Object.fromEntries(
    Object.entries(collections).map(([key, { id: _id, ...collection }]) => [key, collection]),
  );
}

function softwareElementForPath(
  model: NormalizedSoftwareModel,
  path: string,
  propertyPath: PropertyKey[],
) {
  const element = model.elementsByPath.get(path);
  if (!element) throwAuthoringIssue(propertyPath, "Must reference an existing software-map path");
  return element;
}

const DATA_STORE_KIND_MAP: Record<ModelDataStoreKind, StoreKind> = {
  artifactStore: "document",
  bucket: "document",
  database: "relational",
  fileStore: "document",
  objectStore: "document",
};

function storeKindForDataStore(kind: ModelDataStoreKind | undefined): StoreKind {
  return kind === undefined ? "relational" : DATA_STORE_KIND_MAP[kind];
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
      const authored = Object.assign(
        {},
        defineFieldTargets(target, collection.schema, []),
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
      const nested = isNestedSchema(value) ? value : value.schema;
      const authored = Object.assign(
        {},
        nested ? defineFieldTargets(collection, nested, path) : {},
      ) as AuthoredTargetRef & Record<PropertyKey, unknown>;
      Object.defineProperty(authored, authoredTargetRefKey, { value: target });
      return [field, Object.freeze(authored)];
    }),
  );
}

function isNestedSchema(
  value: SoftwareDataStoreFieldSchema[string],
): value is SoftwareDataStoreFieldSchema {
  return typeof (value as { type?: unknown }).type !== "string";
}

export function throwAuthoringIssue(path: PropertyKey[], message: string): never {
  throw new z.ZodError([{ code: "custom", path, message, input: undefined }]);
}

function errorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message;
  if (typeof error === "string" && error.trim()) return error;
  return String(error);
}

// Keeps the React type in the public authoring surface, matching Review's
// component-prop contracts without shipping a second presentation model.
export type ReviewDocumentChildren = ReactNode;
