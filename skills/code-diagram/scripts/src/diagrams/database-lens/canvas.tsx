import { useMemo, useReducer } from "react";
import type { ActorRef } from "../../authoring/core";
import { type CollectionKind, type TargetRef } from "./authoring";
import {
  flattenDataStoreSchema,
  formatDataStoreExample,
  parseDataStoreForeignKeyRef,
} from "../../canvas/data-store-schema";
import {
  DiagramCanvas,
} from "../../canvas/c4/canvas";
import type {
  DiagramDataStoreSchemaRow,
  DiagramNode,
  DiagramSnapshot,
} from "../../canvas/model";
import {
  canvasInteractionReducer,
  createCanvasInteractionState,
} from "../../canvas/interaction";
import type { EmphasisState, OpenEvidence } from "../../document/model";
import {
  compiledDatabaseLensSchema,
  type CompiledDatabaseLens,
} from "./model";
import "./styles.css";

type DatabaseStore = CompiledDatabaseLens["stores"][number];
type DatabaseCollection = DatabaseStore["collections"][number];

interface ResolvedOperation {
  relationshipId: string;
  operation: CompiledDatabaseLens["useCases"][number]["operations"][number];
}

type DatabaseOperationHighlights = {
  operationStates: Map<string, Exclude<EmphasisState, "normal">>;
  activeTargetKey?: string;
};

export function DatabaseLensCanvas({
  model,
  openEvidence,
}: {
  model: CompiledDatabaseLens;
  openEvidence: OpenEvidence;
}) {
  const { height = 560, stores, useCases } = model;
  const { resolvedOperations, highlights } = useMemo(() => {
    const resolvedOperations = useCases
      .flatMap((item) => item.operations)
      .map((operation, index) => ({
        relationshipId: `all:${operation.anchor.id}:${index}`,
        operation,
      }));
    const activeTarget = resolvedOperations[0]?.operation.target;
    return {
      resolvedOperations,
      highlights: {
        operationStates: new Map(
          resolvedOperations.map(({ relationshipId }, index) => [
            relationshipId,
            index === 0 ? "active" : "inactive",
          ]),
        ),
        activeTargetKey: activeTarget
          ? `${activeTarget.storeId}.${activeTarget.collectionKind}.${activeTarget.collectionId}.${activeTarget.path.join(".")}`
          : undefined,
      } satisfies DatabaseOperationHighlights,
    };
  }, [useCases]);
  const canvasHeight = Math.max(height, 360 + useCases.length * 180);
  return (
    <div
      className="database-lens diagram-design-db-schema"
      data-diagram-design-type="database-schema"
    >
      <div className="database-lens-diagram" style={{ height: canvasHeight }}>
        <DatabaseUseCaseCanvas
          stores={stores}
          resolvedOperations={resolvedOperations}
          highlights={highlights}
          openEvidence={openEvidence}
        />
      </div>
    </div>
  );
}

function DatabaseUseCaseCanvas({
  stores,
  resolvedOperations,
  highlights,
  openEvidence,
}: {
  stores: DatabaseStore[];
  resolvedOperations: ResolvedOperation[];
  highlights: DatabaseOperationHighlights;
  openEvidence: OpenEvidence;
}) {
  const defaultExpandedNodeIds = useMemo(
    () =>
      new Set(
        resolvedOperations.map(({ operation }) =>
          storeNodeId(operation.target),
        ),
      ),
    [resolvedOperations],
  );
  const [interaction, dispatchInteraction] = useReducer(
    canvasInteractionReducer,
    defaultExpandedNodeIds,
    createCanvasInteractionState,
  );
  const { expandedNodeIds, selectedNodeId, viewportFocusNodeId } = interaction;
  const snapshot = useMemo(
    () =>
      databaseDiagramSnapshot({
        stores,
        resolvedOperations,
        highlights,
        selectedNodeId,
        expandedNodeIds,
      }),
    [
      stores,
      resolvedOperations,
      highlights,
      selectedNodeId,
      expandedNodeIds,
    ],
  );
  const selectedNodeIdForFrame =
    selectedNodeId && snapshot.nodes.some((node) => node.id === selectedNodeId)
      ? selectedNodeId
      : (snapshot.selectedNodeId ?? snapshot.nodes[0]?.id ?? null);
  const frameSnapshot = {
    ...snapshot,
    selectedNodeId: selectedNodeIdForFrame,
  };
  const openRelationship = (relationshipId: string) => {
    const operation = resolvedOperations.find(
      (resolved) => resolved.relationshipId === relationshipId,
    )?.operation;
    if (!operation) return;
    openEvidence(operation.source, operation.anchor.title);
  };
  const handleSelectNode = (node: DiagramNode) => {
    dispatchInteraction({ type: "select", nodeId: node.id });
  };
  const handleToggleNodeExpansion = (node: DiagramNode) => {
    if (!node.expandable) return;
    dispatchInteraction(
      node.expanded
        ? { type: "collapse", nodeId: node.id }
        : { type: "expand", nodeId: node.id, focus: true },
    );
  };
  const handleExpandNode = (node: DiagramNode) => {
    if (!node.expandable) return;
    dispatchInteraction({ type: "expand", nodeId: node.id, focus: true });
  };
  const handleCollapseNode = (node: DiagramNode) => {
    dispatchInteraction({ type: "collapse", nodeId: node.id });
  };
  return (
    <div className="database-diagram-canvas database-diagram-canvas--c4">
      <DiagramCanvas
        snapshot={frameSnapshot}
        height="100%"
        onSelectNode={handleSelectNode}
        onExpandNode={handleExpandNode}
        onCollapseNode={handleCollapseNode}
        onToggleNodeExpansion={handleToggleNodeExpansion}
        onFocusNode={(node) =>
          dispatchInteraction({ type: "focus", nodeId: node.id })
        }
        relationshipStateById={highlights.operationStates}
        onOpenRelationship={openRelationship}
        viewportFocusNodeId={viewportFocusNodeId}
        onViewportFocusComplete={(nodeId) =>
          dispatchInteraction({ type: "focus-complete", nodeId })
        }
      />
    </div>
  );
}

function databaseDiagramSnapshot({
  stores,
  resolvedOperations,
  highlights,
  selectedNodeId,
  expandedNodeIds,
}: {
  stores: DatabaseStore[];
  resolvedOperations: ResolvedOperation[];
  highlights: DatabaseOperationHighlights;
  selectedNodeId: string | null;
  expandedNodeIds: ReadonlySet<string>;
}): DiagramSnapshot {
  const nodes = new Map<string, DiagramNode>();
  const relationships: DiagramSnapshot["relationships"] = [];
  const expandedStoresWithSchemaEdges = new Set<string>();
  for (const resolved of resolvedOperations) {
    const { actor, target } = resolved.operation;
    const actorId = actorNodeId(actor);
    const storeId = storeNodeId(target);
    const storeExpanded = expandedNodeIds.has(storeId);
    const operationStore = stores.find((store) => store.id === target.storeId);
    const targetNodeId = storeExpanded
      ? storeCollectionNodeId(target)
      : storeId;
    nodes.set(actorId, softwareMapNodeForActor(actor));
    nodes.set(
      storeId,
      softwareMapNodeForStore({
        target,
        store: operationStore,
        expanded: storeExpanded,
      }),
    );
    if (storeExpanded && operationStore) {
      for (const collection of operationStore.collections) {
        const node = softwareMapCollectionNode({
          store: operationStore,
          storeNodeId: storeId,
          collection,
          highlights,
        });
        nodes.set(node.id, node);
      }
      if (!expandedStoresWithSchemaEdges.has(operationStore.id)) {
        relationships.push(
          ...softwareMapForeignKeyRelationshipsForStore(operationStore),
        );
        expandedStoresWithSchemaEdges.add(operationStore.id);
      }
    }
    relationships.push({
      id: resolved.relationshipId,
      from: resolved.operation.kind === "write" ? actorId : targetNodeId,
      to: resolved.operation.kind === "write" ? targetNodeId : actorId,
      kind: "semantic",
      semanticKind: resolved.operation.kind,
      label: resolved.operation.label,
      ...(storeExpanded && resolved.operation.kind === "write"
        ? {
            toSchemaFieldPath: target.path,
            toSchemaEndpointKind: "field" as const,
          }
        : {}),
      ...(storeExpanded && resolved.operation.kind === "read"
        ? {
            fromSchemaFieldPath: target.path,
            fromSchemaEndpointKind: "field" as const,
          }
        : {}),
    });
  }
  const activeTarget = resolvedOperations[0]?.operation.target;
  return {
    view: "database:all",
    nodes: [...nodes.values()],
    relationships,
    selectedNodeId:
      selectedNodeId ?? (activeTarget ? storeNodeId(activeTarget) : undefined),
  };
}

function softwareMapNodeForActor(actor: ActorRef): DiagramNode {
  return {
    id: actorNodeId(actor),
    type: "component",
    label: actor.label,
    path: actor.softwareMapPath,
  };
}

function softwareMapNodeForStore({
  target,
  store,
  expanded,
}: {
  target: TargetRef;
  store: DatabaseStore | undefined;
  expanded: boolean;
}): DiagramNode {
  const childCount = store?.collections.length ?? 0;
  const id = storeNodeId(target);
  return {
    id,
    type: "dataStore",
    label: target.storeLabel,
    path: target.storeSoftwareMapPath,
    description: target.collectionLabel,
    dataStoreKind:
      target.storeDataStoreKind ??
      (target.storeKind === "relational" ? "database" : "artifactStore"),
    expanded,
    expandable: childCount > 0,
    childCount,
  };
}

function softwareMapCollectionNode({
  store,
  storeNodeId,
  collection,
  highlights,
}: {
  store: DatabaseStore;
  storeNodeId: string;
  collection: DatabaseCollection;
  highlights: DatabaseOperationHighlights;
}): DiagramNode {
  const kind = collection.kind === "tables" ? "table" : "document";
  return {
    id: storeCollectionNodeIdForStore(store, collection.kind, collection.id),
    type: "dataStoreCollection",
    label: collection.label,
    path: store.softwareMapPath
      ? `${store.softwareMapPath}.${collection.kind}.${collection.id}`
      : undefined,
    description: kind === "table" ? "Table" : "Document",
    parentId: storeNodeId,
    dataStoreSchemaSections: [
      {
        id: `${kind}:${collection.id}`,
        kind,
        label: collection.label,
        key: collection.key,
        rows: softwareMapSchemaRowsForCollection({
          store,
          collection,
          highlights,
        }),
      },
    ],
  };
}

function softwareMapForeignKeyRelationshipsForStore(
  store: DatabaseStore,
): DiagramSnapshot["relationships"] {
  const relationships: DiagramSnapshot["relationships"] = [];
  for (const collection of store.collections) {
    if (collection.kind !== "tables") continue;
    const sourceCollectionNodeId = storeCollectionNodeIdForStore(
      store,
      "tables",
      collection.id,
    );
    for (const row of flattenDataStoreSchema(collection.schema)) {
      if (!row.fk) continue;
      const target = parseDataStoreForeignKeyRef(row.fk);
      if (
        !target ||
        !store.collections.some(
          ({ id, kind }) => kind === "tables" && id === target.table,
        )
      )
        continue;
      const targetCollectionNodeId = storeCollectionNodeIdForStore(
        store,
        "tables",
        target.table,
      );
      if (targetCollectionNodeId === sourceCollectionNodeId) continue;
      const id = `schema-fk:${sourceCollectionNodeId}.${row.path.join(".")}->${targetCollectionNodeId}.${target.fieldPath.join(".")}`;
      relationships.push({
        id,
        from: sourceCollectionNodeId,
        to: targetCollectionNodeId,
        kind: "semantic",
        semanticKind: "foreign-key",
        hideLabel: true,
        fromSchemaFieldPath: row.path,
        fromSchemaEndpointKind: "field",
        toSchemaFieldPath: [],
        toSchemaEndpointKind: "header",
      });
    }
  }
  return relationships;
}

function softwareMapSchemaRowsForCollection({
  store,
  collection,
  highlights,
}: {
  store: DatabaseStore;
  collection: DatabaseCollection;
  highlights: DatabaseOperationHighlights;
}): DiagramDataStoreSchemaRow[] {
  return flattenDataStoreSchema(collection.schema).map((row) => {
    const rowTargetKey = `${store.id}.${collection.kind}.${collection.id}.${row.path.join(".")}`;
    return {
      id: `${collection.id}:${row.path.join(".")}`,
      label: row.label,
      depth: row.depth,
      type: row.type ?? "object",
      example: formatDataStoreExample(row.example),
      primaryKey: row.pk,
      foreignKey: Boolean(row.fk),
      state: highlights.activeTargetKey === rowTargetKey
        ? "active"
        : "inactive",
    };
  });
}

function storeNodeId(target: TargetRef): string {
  return `store:${target.storeId}`;
}

function storeCollectionNodeId(target: TargetRef): string {
  return target.storeSoftwareMapPath
    ? `${target.storeSoftwareMapPath}.${target.collectionKind}.${target.collectionId}`
    : `store:${target.storeId}.${target.collectionKind}.${target.collectionId}`;
}

function storeCollectionNodeIdForStore(
  store: DatabaseStore,
  collectionKind: CollectionKind,
  collectionId: string,
): string {
  return store.softwareMapPath
    ? `${store.softwareMapPath}.${collectionKind}.${collectionId}`
    : `store:${store.id}.${collectionKind}.${collectionId}`;
}

function actorNodeId(actor: ActorRef): string {
  return `actor:${actor.id}`;
}

export { DatabaseLensCanvas as Renderer, compiledDatabaseLensSchema as schema };
