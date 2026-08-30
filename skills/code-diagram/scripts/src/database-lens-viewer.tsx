import React from "react";
import type { AuthoredTargetRef, StoreInputMap, StoreRef } from "./authoring";
import { createReviewDefinitionSession } from "./authoring";
import type { CompiledDatabaseLens } from "./database-lens-model";
import type { SourceRange } from "./diagram-family";
import {
  DatabaseLens,
  DbRead,
  DbUseCase,
  DbWrite,
} from "./review-runtime/app/src/database-lens";
import { OfflineReviewRuntime } from "./review-runtime/app/src/offline-context";

export function DatabaseLensRenderer({
  model,
  openEvidence,
}: {
  model: CompiledDatabaseLens;
  openEvidence: (source: SourceRange | readonly SourceRange[], title?: string) => void;
}) {
  const stores = createReviewDefinitionSession({}).defineStores(storeInputs(model));
  const sourcesByAnchorId = Object.fromEntries(
    model.useCases.flatMap((useCase) =>
      useCase.operations.map((operation) => [operation.anchor.id, operation.source]),
    ),
  );
  const children = model.useCases.map((useCase) =>
    React.createElement(
      DbUseCase,
      {
        key: useCase.id,
        id: useCase.id,
        label: useCase.label,
        summary: useCase.summary,
        children: undefined,
      },
      ...useCase.operations.map((operation, index) => {
        const target = targetRef(stores, operation.target);
        return operation.kind === "read"
          ? React.createElement(DbRead, {
              key: `${operation.anchor.id}-${index}`,
              from: target,
              to: operation.actor,
              label: operation.label,
              anchor: operation.anchor,
            })
          : React.createElement(DbWrite, {
              key: `${operation.anchor.id}-${index}`,
              from: operation.actor,
              to: target,
              label: operation.label,
              anchor: operation.anchor,
            });
      }),
    ),
  );
  return (
    <OfflineReviewRuntime openEvidence={openEvidence} sourcesByAnchorId={sourcesByAnchorId}>
      <DatabaseLens title={model.title} stores={stores} height={model.height}>
        {children}
      </DatabaseLens>
    </OfflineReviewRuntime>
  );
}

function storeInputs(model: CompiledDatabaseLens): StoreInputMap {
  return Object.fromEntries(
    model.stores.map((store) => {
      const collections = (kind: "tables" | "documents") =>
        Object.fromEntries(
          store.collections
            .filter((collection) => collection.kind === kind)
            .map((collection) => [
              collection.id,
              {
                label: collection.label,
                key: collection.key,
                schema: collection.schema,
              },
            ]),
        );
      const tables = collections("tables");
      const documents = collections("documents");
      return [
        store.id,
        {
          kind: store.kind,
          label: store.label,
          dataStoreKind: store.dataStoreKind,
          softwareMapPath: store.softwareMapPath,
          ...(Object.keys(tables).length ? { tables } : {}),
          ...(Object.keys(documents).length ? { documents } : {}),
        },
      ];
    }),
  ) as StoreInputMap;
}

function targetRef(
  stores: Record<string, StoreRef>,
  target: CompiledDatabaseLens["useCases"][number]["operations"][number]["target"],
): AuthoredTargetRef {
  const store = stores[target.storeId];
  const collection = store?.[target.collectionKind]?.[target.collectionId];
  if (!collection) throw new Error(`Missing database target ${target.storeId}.${target.collectionId}`);
  let ref: unknown = collection;
  for (const part of target.path) ref = (ref as Record<string, unknown>)[part];
  return ref as AuthoredTargetRef;
}
