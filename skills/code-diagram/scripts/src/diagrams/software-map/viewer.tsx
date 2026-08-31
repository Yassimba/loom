import { useMemo, useReducer } from "react";
import { DiagramCanvas } from "../../canvas/c4/canvas";
import type {
  CanvasRelationshipKind,
  DiagramNode,
  DiagramRelationship,
} from "../../canvas/model";
import {
  canvasInteractionReducer,
  createCanvasInteractionState,
} from "../../canvas/interaction";
import {
  isCanvasRelationshipSemanticKind as isSoftwareRelationshipSemanticKind,
} from "../../canvas/semantics";
import type { DiagramRendererProps } from "../../document/model";
import {
  buildSoftwareMapChangeSummaries,
  initialSoftwareMapExpandedNodeIds,
  softwareMapSnapshotFromInlineC4Projection,
} from "./c4-adapter";
import {
  compiledSoftwareMapSchema,
  type CompiledSoftwareMap,
} from "./schema";
import type {
  NormalizedSoftwareModel,
  NormalizedSoftwareRelationship,
  SoftwareRelationshipSemanticKind,
} from "./model";
import { collapseInlineC4Node, projectInlineC4 } from "./projection";

export function SoftwareMapRenderer({
  model,
  openEvidence,
}: DiagramRendererProps<CompiledSoftwareMap>) {
  const normalized = useMemo(() => normalizedModel(model), [model]);
  const [interaction, dispatchInteraction] = useReducer(
    canvasInteractionReducer,
    initialSoftwareMapExpandedNodeIds(normalized),
    createCanvasInteractionState,
  );
  const { expandedNodeIds, selectedNodeId } = interaction;
  const projection = useMemo(
    () => projectInlineC4({ model: normalized, expandedNodeIds, selectedNodeId: selectedNodeId ?? undefined }),
    [expandedNodeIds, normalized, selectedNodeId],
  );
  const changeSummaries = useMemo(
    () =>
      buildSoftwareMapChangeSummaries(
        normalized,
        model.diffCountsByPath,
      ),
    [model.diffCountsByPath, normalized],
  );
  const snapshot = useMemo(
    () => softwareMapSnapshotFromInlineC4Projection({ projection, changeSummaries }),
    [changeSummaries, projection],
  );
  const openNodeEvidence = (node: DiagramNode) => {
    if (!node.path) return;
    const sources = Object.entries(model.evidenceByPath)
      .filter(
        ([path]) => path === node.path || path.startsWith(`${node.path}.`),
      )
      .flatMap(([, ranges]) => ranges);
    if (sources.length) openEvidence(sources, node.label);
  };
  const selectNode = (node: DiagramNode) => {
    dispatchInteraction({ type: "select", nodeId: node.id });
    openNodeEvidence(node);
  };
  const expandNode = (node: DiagramNode) => {
    dispatchInteraction({ type: "expand", nodeId: node.id });
    openNodeEvidence(node);
  };
  const collapseNode = (node: DiagramNode) => {
    dispatchInteraction({
      type: "collapse",
      nodeId: node.id,
      expandedNodeIds: collapseInlineC4Node(expandedNodeIds, node.id),
    });
  };
  const toggleNode = (node: DiagramNode) => {
    if (node.expanded) collapseNode(node);
    else expandNode(node);
  };
  return (
    <section
      className="software-map diagram-design-architecture"
      data-diagram-design-type="architecture"
      aria-label="Software map"
    >
      <DiagramCanvas
        snapshot={snapshot}
        onSelectNode={selectNode}
        onExpandNode={expandNode}
        onCollapseNode={collapseNode}
        onToggleNodeExpansion={toggleNode}
        onOpenRelationship={(relationshipId) => {
          const relationship = projection.relationships.find(
            ({ id }) => id === relationshipId,
          );
          const sources = relationship?.sourceRelationshipIds.flatMap(
            (id) => model.evidenceByRelationshipId[id] ?? [],
          );
          if (sources?.length) openEvidence(sources, "Relationship evidence");
        }}
      />
      <SoftwareMapLegend relationships={snapshot.relationships} />
    </section>
  );
}

type SoftwareMapLegendKind =
  | CanvasRelationshipKind
  | SoftwareRelationshipSemanticKind;

const relationshipLegend: ReadonlyArray<{
  kind: SoftwareMapLegendKind;
  label: string;
}> = [
  { kind: "call", label: "Call" },
  { kind: "dependency", label: "Dependency" },
  { kind: "http", label: "HTTP / API" },
  { kind: "async", label: "Async / event" },
  { kind: "return", label: "Return" },
  { kind: "optional", label: "Optional / passive" },
  { kind: "primary", label: "Primary flow" },
  { kind: "forbidden", label: "Blocked path" },
  { kind: "published", label: "Published output" },
  { kind: "foreign-key", label: "Foreign key" },
  { kind: "semantic", label: "Declared relationship" },
];

function SoftwareMapLegend({
  relationships,
}: {
  relationships: readonly DiagramRelationship[];
}) {
  const visibleKinds = new Set(
    relationships.map((relationship): SoftwareMapLegendKind => {
      const kind = relationship.kind;
      if (kind !== "semantic") return kind;
      return isSoftwareRelationshipSemanticKind(relationship.semanticKind)
        ? relationship.semanticKind
        : kind;
    }),
  );
  const entries = relationshipLegend.filter(({ kind }) => visibleKinds.has(kind));
  if (entries.length === 0) return null;
  return (
    <aside className="software-map-legend" aria-label="Relationship legend">
      <span className="software-map-legend-title">Legend</span>
      <ul>
        {entries.map(({ kind, label }) => (
          <li key={kind}>
            <span
              className={`software-map-legend-arrow software-map-legend-arrow--${kind}`}
              aria-hidden="true"
            />
            <span>{label}</span>
          </li>
        ))}
      </ul>
    </aside>
  );
}

function normalizedModel(model: CompiledSoftwareMap): NormalizedSoftwareModel {
  const elements = model.elements as unknown as NormalizedSoftwareModel["elements"];
  return {
    elements,
    elementsByPath: new Map(elements.map((element) => [element.path, element])),
    relationships: model.relationships as unknown as NormalizedSoftwareRelationship[],
  };
}

export { SoftwareMapRenderer as Renderer, compiledSoftwareMapSchema as schema };
