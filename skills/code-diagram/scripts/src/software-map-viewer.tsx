import { useMemo, useState } from "react";
import { createPortal } from "react-dom";
import type { SourceRange } from "./diagram-family";
import { OfflineReviewRuntime } from "./review-runtime/app/src/offline-context";
import {
  buildSoftwareMapChangeSummaries,
  initialSoftwareMapExpandedNodeIds,
  SoftwareMapFrame,
  softwareMapOverlayClassName,
  softwareMapSnapshotFromInlineC4Projection,
  type SoftwareMapNodeSnapshot,
} from "./review-runtime/app/src/software-map/SoftwareMap";
import {
  collapseInlineC4Node,
  projectInlineC4,
} from "./review-runtime/app/src/software-map/c4-projection";
import type { NormalizedSoftwareModel, NormalizedSoftwareRelationship } from "./software-map-model";
import type { CompiledSoftwareMap } from "./software-map-schema";

export function SoftwareMapRenderer({
  model,
  openEvidence,
}: {
  model: CompiledSoftwareMap;
  openEvidence: (source: SourceRange | readonly SourceRange[], title?: string) => void;
}) {
  const normalized = useMemo(() => normalizedModel(model), [model]);
  const [expandedNodeIds, setExpandedNodeIds] = useState<Set<string>>(() =>
    initialSoftwareMapExpandedNodeIds(normalized),
  );
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [fullscreen, setFullscreen] = useState(false);
  const projection = useMemo(
    () => projectInlineC4({ model: normalized, expandedNodeIds, selectedNodeId: selectedNodeId ?? undefined }),
    [expandedNodeIds, normalized, selectedNodeId],
  );
  const changeSummaries = useMemo(
    () =>
      buildSoftwareMapChangeSummaries(
        normalized,
        new Map(Object.entries(model.diffCountsByPath)),
      ),
    [model.diffCountsByPath, normalized],
  );
  const snapshot = useMemo(
    () => softwareMapSnapshotFromInlineC4Projection({ projection, changeSummaries }),
    [changeSummaries, projection],
  );
  const evidenceByProjectedRelationshipId = useMemo(
    () =>
      Object.fromEntries(
        projection.relationships.map((relationship) => [
          relationship.id,
          relationship.sourceRelationshipIds.flatMap(
            (id) => model.evidenceByRelationshipId[id] ?? [],
          ),
        ]),
      ),
    [model.evidenceByRelationshipId, projection.relationships],
  );
  const selectNode = (node: SoftwareMapNodeSnapshot) => {
    setSelectedNodeId(node.id);
    if (!node.path) return;
    const sources = Object.entries(model.evidenceByPath)
      .filter(([path]) => path === node.path || path.startsWith(`${node.path}.`))
      .flatMap(([, ranges]) => ranges);
    if (sources.length) openEvidence(sources, node.label);
  };
  const expandNode = (node: SoftwareMapNodeSnapshot) => {
    setExpandedNodeIds((current) => new Set(current).add(node.id));
    selectNode(node);
  };
  const collapseNode = (node: SoftwareMapNodeSnapshot) => {
    setExpandedNodeIds((current) => collapseInlineC4Node(current, node.id));
    setSelectedNodeId(node.id);
  };
  const toggleNode = (node: SoftwareMapNodeSnapshot) => {
    if (node.expanded) collapseNode(node);
    else expandNode(node);
  };
  const frameProps = {
    snapshot,
    hasResolvedSnapshot: true,
    title: "Software map",
    viewName: "inline-c4",
    status: null,
    error: null,
    refreshing: false,
    showChrome: true,
    showFloatingActions: true,
    inspectedNode: null,
    onSelectNode: selectNode,
    onExpandNode: expandNode,
    onCollapseNode: collapseNode,
    onToggleNodeExpansion: toggleNode,
    onOpenRelationship: (relationshipId: string) => {
      const sources = evidenceByProjectedRelationshipId[relationshipId];
      if (sources?.length) openEvidence(sources, "Relationship evidence");
    },
  };
  return (
    <OfflineReviewRuntime openEvidence={openEvidence}>
      <section className="software-map" aria-label="Software map">
        <SoftwareMapFrame
          {...frameProps}
          expanded={false}
          interactionMode="inline"
          onExpand={() => setFullscreen(true)}
        />
        {fullscreen && typeof document !== "undefined"
          ? createPortal(
              <div
                className={softwareMapOverlayClassName({ theme: "dark", nodeTint: "none" })}
                role="dialog"
                aria-modal="true"
                aria-label="Software map expanded"
              >
                <SoftwareMapFrame
                  {...frameProps}
                  expanded
                  interactionMode="standalone"
                  onClose={() => setFullscreen(false)}
                />
              </div>,
              document.body,
            )
          : null}
      </section>
    </OfflineReviewRuntime>
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
