import {
  flattenDataStoreSchema,
  formatDataStoreExample,
  parseDataStoreForeignKeyRef,
} from "../../canvas/data-store-schema";
import { parseDataStoreSchemaEndpoint } from "./model";
import type {
  NormalizedSoftwareDataStoreCollection,
  NormalizedSoftwareElement,
  NormalizedSoftwareModel,
  NormalizedSoftwareRelationship,
  SoftwareChangeStatus,
  SoftwareDataStoreKind,
  SoftwareElementType,
} from "./model";

export interface C4ProjectionInput {
  model: NormalizedSoftwareModel;
  expandedNodeIds: ReadonlySet<string>;
  selectedNodeId?: string;
}

export interface ProjectedC4Node {
  id: string;
  path: string;
  type: ProjectedC4NodeType;
  label: string;
  description?: string;
  changeStatus?: SoftwareChangeStatus;
  external?: boolean;
  dataStoreKind?: SoftwareDataStoreKind;
  parentPath?: string;
  childCount: number;
  dataStoreSchemaSections?: ProjectedC4DataStoreSchemaSection[];
  isExpanded: boolean;
  isExpandable: boolean;
  element?: NormalizedSoftwareElement;
}

export type ProjectedC4NodeType = SoftwareElementType | "dataStoreCollection";

export interface ProjectedC4DataStoreSchemaSection {
  id: string;
  label: string;
  kind: "table" | "document";
  key?: string;
  rows: ProjectedC4DataStoreSchemaRow[];
}

export interface ProjectedC4DataStoreSchemaRow {
  id: string;
  label: string;
  depth?: number;
  type?: string;
  example?: string;
  primaryKey?: boolean;
  foreignKey?: boolean;
}

export interface ProjectedC4Relationship {
  id: string;
  kind: NormalizedSoftwareRelationship["kind"];
  from: string;
  to: string;
  label?: string;
  semanticKind?: string;
  sourceRelationshipIds: string[];
  hideLabel?: boolean;
  fromSchemaFieldPath?: string[];
  toSchemaFieldPath?: string[];
  fromSchemaEndpointKind?: "field" | "header";
  toSchemaEndpointKind?: "field" | "header";
}

export interface C4Projection {
  nodes: ProjectedC4Node[];
  relationships: ProjectedC4Relationship[];
  selectedNodeId?: string;
}

interface RelationshipBucket {
  kind: NormalizedSoftwareRelationship["kind"];
  from: string;
  to: string;
  relationships: NormalizedSoftwareRelationship[];
}

export function projectInlineC4({
  model,
  expandedNodeIds,
  selectedNodeId,
}: C4ProjectionInput): C4Projection {
  const visibleNodeIds = visibleNodeIdsForProjection(model, expandedNodeIds);

  const nodes = model.elements.flatMap((element) => {
    if (!visibleNodeIds.has(element.path)) return [];
    const isExpandable = isInlineC4Expandable(element);
    const isExpanded = isExpandable && expandedNodeIds.has(element.path);
    const node = projectNode(element, isExpanded, isExpandable);
    return [
      node,
      ...(isExpanded ? projectDataStoreCollectionNodes(element) : []),
    ];
  });

  return {
    nodes,
    relationships: [
      ...projectRelationships(model, visibleNodeIds),
      ...projectDataStoreForeignKeyRelationships(model, visibleNodeIds),
    ],
    selectedNodeId,
  };
}

export function collapseInlineC4Node(
  expandedNodeIds: ReadonlySet<string>,
  nodeId: string,
): Set<string> {
  const collapsed = new Set(expandedNodeIds);
  for (const expandedNodeId of expandedNodeIds) {
    if (expandedNodeId === nodeId || isDescendantPath(expandedNodeId, nodeId)) {
      collapsed.delete(expandedNodeId);
    }
  }
  return collapsed;
}

function visibleNodeIdsForProjection(
  model: NormalizedSoftwareModel,
  expandedNodeIds: ReadonlySet<string>,
) {
  const visibleNodeIds = new Set<string>();
  for (const rootNode of model.elements) {
    if (rootNode.parentPath) continue;
    addVisibleSubtree(model, rootNode, expandedNodeIds, visibleNodeIds);
  }

  return visibleNodeIds;
}

function addVisibleSubtree(
  model: NormalizedSoftwareModel,
  element: NormalizedSoftwareElement,
  expandedNodeIds: ReadonlySet<string>,
  visibleNodeIds: Set<string>,
) {
  visibleNodeIds.add(element.path);
  if (!expandedNodeIds.has(element.path) || !isInlineC4Expandable(element)) {
    return;
  }

  if (element.type === "dataStore") {
    for (const childPath of dataStoreCollectionPaths(element)) {
      visibleNodeIds.add(childPath);
    }
  }

  for (const childPath of element.children) {
    const child = model.elementsByPath.get(childPath)!;
    addVisibleSubtree(model, child, expandedNodeIds, visibleNodeIds);
  }
}

function projectNode(
  element: NormalizedSoftwareElement,
  isExpanded: boolean,
  isExpandable: boolean,
): ProjectedC4Node {
  return {
    id: element.path,
    path: element.path,
    type: element.type,
    label: element.label,
    description: element.description,
    changeStatus: element.changeStatus,
    external: element.external,
    dataStoreKind: element.dataStoreKind,
    parentPath: element.parentPath,
    childCount: element.children.length + dataStoreSchemaChildCount(element),
    isExpanded,
    isExpandable,
    element,
  };
}

function projectDataStoreCollectionNodes(
  element: NormalizedSoftwareElement,
): ProjectedC4Node[] {
  if (element.type !== "dataStore" || !element.dataStoreSchema) return [];
  return dataStoreSchemaSectionsForElement(element).map((section) => {
    const collectionPath = dataStoreCollectionPath(
      element.path,
      section.kind === "table" ? "tables" : "documents",
      section.id.slice(section.kind.length + 1),
    );
    return {
      id: collectionPath,
      path: collectionPath,
      type: "dataStoreCollection",
      label: section.label,
      description: section.kind === "table" ? "Table" : "Document",
      parentPath: element.path,
      childCount: 0,
      dataStoreSchemaSections: [section],
      isExpanded: false,
      isExpandable: false,
    };
  });
}

function dataStoreSchemaChildCount(element: NormalizedSoftwareElement) {
  if (element.type !== "dataStore" || !element.dataStoreSchema) return 0;
  return (
    Object.keys(element.dataStoreSchema.tables).length +
    Object.keys(element.dataStoreSchema.documents).length
  );
}

function dataStoreSchemaSectionsForElement(
  element: NormalizedSoftwareElement,
): ProjectedC4DataStoreSchemaSection[] {
  if (!element.dataStoreSchema) return [];
  const sections: ProjectedC4DataStoreSchemaSection[] = [];
  for (const collection of Object.values(element.dataStoreSchema.tables)) {
    sections.push(dataStoreSchemaSection("table", collection));
  }
  for (const collection of Object.values(element.dataStoreSchema.documents)) {
    sections.push(dataStoreSchemaSection("document", collection));
  }
  return sections;
}

function dataStoreSchemaSection(
  kind: "table" | "document",
  collection: NormalizedSoftwareDataStoreCollection,
): ProjectedC4DataStoreSchemaSection {
  return {
    id: `${kind}:${collection.id}`,
    kind,
    label: collection.label,
    key: collection.key,
    rows: flattenDataStoreSchema(collection.schema).map((row) => ({
      id: `${collection.id}:${row.path.join(".")}`,
      label: row.label,
      depth: row.depth,
      type: row.type ?? "object",
      example: formatDataStoreExample(row.example),
      primaryKey: row.pk,
      foreignKey: Boolean(row.fk),
    })),
  };
}

function dataStoreCollectionPaths(
  element: NormalizedSoftwareElement,
): string[] {
  if (element.type !== "dataStore" || !element.dataStoreSchema) return [];
  return [
    ...Object.keys(element.dataStoreSchema.tables).map((collectionId) =>
      dataStoreCollectionPath(element.path, "tables", collectionId),
    ),
    ...Object.keys(element.dataStoreSchema.documents).map((collectionId) =>
      dataStoreCollectionPath(element.path, "documents", collectionId),
    ),
  ];
}

function dataStoreCollectionPath(
  dataStorePath: string,
  collectionKind: "tables" | "documents",
  collectionId: string,
) {
  return `${dataStorePath}.${collectionKind}.${collectionId}`;
}

function parsedSchemaEndpoint(
  model: NormalizedSoftwareModel,
  endpoint: string,
) {
  return looksLikeDataStoreSchemaEndpoint(endpoint)
    ? parseDataStoreSchemaEndpoint(endpoint, model.elementsByPath)
    : undefined;
}

function projectRelationships(
  model: NormalizedSoftwareModel,
  visibleNodeIds: ReadonlySet<string>,
) {
  const buckets = new Map<string, RelationshipBucket>();

  for (const relationship of model.relationships) {
    const from = projectedEndpoint(model, relationship.from, visibleNodeIds);
    const to = projectedEndpoint(model, relationship.to, visibleNodeIds);
    if (!from || !to || from === to) continue;

    const separatesKind =
      model.elementsByPath.get(from)?.type === "codeElement" ||
      model.elementsByPath.get(to)?.type === "codeElement";
    const bucketKey = [
      from,
      to,
      separatesKind ? relationship.kind : "",
    ].join("\u0000");
    const bucket = buckets.get(bucketKey) ?? {
      kind: relationship.kind,
      from,
      to,
      relationships: [],
    };

    bucket.relationships.push(relationship);
    buckets.set(bucketKey, bucket);
  }

  return [...buckets.values()].map<ProjectedC4Relationship>((bucket) => {
    const relationship =
      bucket.relationships.length === 1
        ? bucket.relationships[0]
        : undefined;
    return {
      id: `projected:${bucket.from}->${bucket.to}:${bucket.kind}`,
      kind: bucket.kind,
      from: bucket.from,
      to: bucket.to,
      label: relationship?.label,
      semanticKind:
        bucket.kind === "semantic" && relationship?.kind === "semantic"
          ? relationship.semanticKind
          : undefined,
      sourceRelationshipIds: bucket.relationships.map(({ id }) => id),
      ...projectedRelationshipSchemaEndpoints(model, bucket),
    };
  });
}

function projectedRelationshipSchemaEndpoints(
  model: NormalizedSoftwareModel,
  bucket: RelationshipBucket,
): Pick<
  ProjectedC4Relationship,
  | "fromSchemaFieldPath"
  | "toSchemaFieldPath"
  | "fromSchemaEndpointKind"
  | "toSchemaEndpointKind"
> {
  if (bucket.relationships.length !== 1) return {};
  const relationship = bucket.relationships[0]!;
  const fromEndpoint = parsedSchemaEndpoint(model, relationship.from);
  const toEndpoint = parsedSchemaEndpoint(model, relationship.to);
  return {
    ...(fromEndpoint &&
    bucket.from ===
      dataStoreCollectionPath(
        fromEndpoint.dataStorePath,
        fromEndpoint.collectionKind,
        fromEndpoint.collectionId,
      )
      ? {
          fromSchemaFieldPath: fromEndpoint.fieldPath,
          fromSchemaEndpointKind:
            fromEndpoint.fieldPath.length > 0 ? "field" : "header",
        }
      : {}),
    ...(toEndpoint &&
    bucket.to ===
      dataStoreCollectionPath(
        toEndpoint.dataStorePath,
        toEndpoint.collectionKind,
        toEndpoint.collectionId,
      )
      ? {
          toSchemaFieldPath: toEndpoint.fieldPath,
          toSchemaEndpointKind:
            toEndpoint.fieldPath.length > 0 ? "field" : "header",
        }
      : {}),
  };
}

function looksLikeDataStoreSchemaEndpoint(endpoint: string) {
  return endpoint.includes(".tables.") || endpoint.includes(".documents.");
}

function projectDataStoreForeignKeyRelationships(
  model: NormalizedSoftwareModel,
  visibleNodeIds: ReadonlySet<string>,
): ProjectedC4Relationship[] {
  const relationships: ProjectedC4Relationship[] = [];
  for (const element of model.elements) {
    if (element.type !== "dataStore" || !element.dataStoreSchema) continue;
    for (const collection of Object.values(element.dataStoreSchema.tables)) {
      const sourceCollectionPath = dataStoreCollectionPath(
        element.path,
        "tables",
        collection.id,
      );
      if (!visibleNodeIds.has(sourceCollectionPath)) continue;
      for (const row of flattenDataStoreSchema(collection.schema)) {
        if (!row.fk) continue;
        const targetRef = parseDataStoreForeignKeyRef(row.fk);
        if (!targetRef) continue;
        const targetEndpoint = `${element.path}.tables.${targetRef.table}.${targetRef.fieldPath.join(".")}`;
        const target = parsedSchemaEndpoint(model, targetEndpoint);
        if (!target) continue;
        const targetCollectionPath = dataStoreCollectionPath(
          target.dataStorePath,
          target.collectionKind,
          target.collectionId,
        );
        if (
          !visibleNodeIds.has(targetCollectionPath) ||
          targetCollectionPath === sourceCollectionPath
        ) {
          continue;
        }
        const id = `schema-fk:${sourceCollectionPath}.${row.path.join(".")}->${targetEndpoint}`;
        relationships.push({
          id,
          kind: "semantic",
          from: sourceCollectionPath,
          to: targetCollectionPath,
          semanticKind: "foreign-key",
          sourceRelationshipIds: [id],
          hideLabel: true,
          fromSchemaFieldPath: row.path,
          fromSchemaEndpointKind: "field",
          toSchemaFieldPath: [],
          toSchemaEndpointKind: "header",
        });
      }
    }
  }
  return relationships;
}

function projectedEndpoint(
  model: NormalizedSoftwareModel,
  path: string,
  visibleNodeIds: ReadonlySet<string>,
) {
  const schemaEndpoint = parsedSchemaEndpoint(model, path);
  if (schemaEndpoint) {
    const collectionPath = dataStoreCollectionPath(
      schemaEndpoint.dataStorePath,
      schemaEndpoint.collectionKind,
      schemaEndpoint.collectionId,
    );
    if (visibleNodeIds.has(collectionPath)) return collectionPath;
    if (visibleNodeIds.has(schemaEndpoint.dataStorePath)) {
      return schemaEndpoint.dataStorePath;
    }
  }

  let current: string | undefined = path;
  while (current) {
    if (visibleNodeIds.has(current)) return current;
    current = model.elementsByPath.get(current)?.parentPath;
  }
  return undefined;
}

export function isInlineC4Expandable(element: NormalizedSoftwareElement) {
  return (
    element.type !== "codeElement" &&
    (element.children.length > 0 || dataStoreSchemaChildCount(element) > 0)
  );
}

function isDescendantPath(path: string, ancestorPath: string) {
  return path.startsWith(`${ancestorPath}.`);
}
