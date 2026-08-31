import type { ChangeState, EmphasisState } from "../document/model";

export type CanvasNodeKind =
  | "person"
  | "softwareSystem"
  | "container"
  | "dataStore"
  | "dataStoreCollection"
  | "component"
  | "codeElement";

export type CanvasRelationshipKind = "call" | "semantic";

export const CANVAS_DATA_STORE_KINDS = [
  "database",
  "objectStore",
  "bucket",
  "artifactStore",
  "fileStore",
] as const;
export type CanvasDataStoreKind = (typeof CANVAS_DATA_STORE_KINDS)[number];
export type CanvasDataStoreShape = "cylinder" | "bucket" | "folder";

export interface DiagramNode {
  id: string;
  label: string;
  type: CanvasNodeKind;
  path?: string;
  description?: string;
  changeStatus?: ChangeState;
  dataStoreKind?: CanvasDataStoreKind;
  additions?: number;
  deletions?: number;
  parentId?: string | null;
  file?: string;
  line?: number;
  boundary?: boolean;
  expanded?: boolean;
  expandable?: boolean;
  childCount?: number;
  dataStoreSchemaSections?: DiagramDataStoreSchemaSection[];
}

export interface DiagramDataStoreSchemaSection {
  id: string;
  label: string;
  kind: "table" | "document";
  key?: string;
  rows: DiagramDataStoreSchemaRow[];
}

export interface DiagramDataStoreSchemaRow {
  id: string;
  label: string;
  depth?: number;
  type?: string;
  example?: string;
  primaryKey?: boolean;
  foreignKey?: boolean;
  state?: Exclude<EmphasisState, "normal">;
}

export interface DiagramRelationship {
  id: string;
  from: string;
  to: string;
  label?: string;
  kind: CanvasRelationshipKind;
  semanticKind?: string;
  hideLabel?: boolean;
  fromSchemaFieldPath?: string[];
  toSchemaFieldPath?: string[];
  fromSchemaEndpointKind?: "field" | "header";
  toSchemaEndpointKind?: "field" | "header";
}

export interface C4LayoutBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface C4LayoutResult {
  nodeBboxes: Map<string, C4LayoutBox>;
  groupBboxes: Map<string, C4LayoutBox>;
}

export interface DiagramSnapshot {
  view?: string;
  nodes: DiagramNode[];
  relationships: DiagramRelationship[];
  selectedNodeId?: string | null;
}
