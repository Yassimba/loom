import {
  BaseEdge,
  EdgeLabelRenderer,
  Handle,
  MarkerType,
  Position,
  ReactFlow,
  type Edge as ReactFlowEdge,
  type EdgeProps as ReactFlowEdgeProps,
  type ReactFlowInstance,
  type Node as ReactFlowNode,
  type NodeProps as ReactFlowNodeProps,
  type Viewport,
} from "@xyflow/react";
import ELK from "elkjs/lib/elk.bundled.js";
import type { ElkGraph as LibavoidGraph } from "@mr_mint/elkjs-libavoid";
import {
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { ChangeState, EmphasisState } from "../../document/model";
import { hasTextSelectionWithin } from "../../viewer/text-selection";
import {
  edgePointsFromSection,
  polylineMidpoint,
} from "../routing/labels";
import type {
  RoutingLabel,
  RoutingObstacle,
  RoutingSection,
} from "../routing/model";
import { routeOrthogonalConnectors } from "../routing/route";
import { distributePorts, type RoutingSide } from "../routing/ports";
import { isCanvasRelationshipSemanticKind } from "../semantics";
import type {
  C4LayoutBox,
  C4LayoutResult as InlineC4LayoutResult,
  CanvasDataStoreKind,
  CanvasDataStoreShape,
  CanvasNodeKind,
  CanvasRelationshipKind,
  DiagramDataStoreSchemaSection,
  DiagramNode,
  DiagramRelationship,
  DiagramSnapshot,
} from "../model";
import "../styles.css";

interface C4MapNodeData extends Record<string, unknown> {
  node: DiagramNode;
  selected: boolean;
  onSelect?: (node: DiagramNode) => void;
  onExpandNode?: (node: DiagramNode) => void;
  onCollapseNode?: (node: DiagramNode) => void;
}

type C4MapFlowNode = ReactFlowNode<C4MapNodeData, "softwareMapC4">;
type C4MapFlowGroupNode = ReactFlowNode<C4MapNodeData, "softwareMapC4Group">;
type C4MapAnyFlowNode = C4MapFlowNode | C4MapFlowGroupNode;

interface C4DisplayedLayoutState {
  signature: string;
  snapshot: DiagramSnapshot;
  layout: C4LayoutResult;
}

interface C4MapEdgeData extends Record<string, unknown> {
  relationship: DiagramRelationship;
  selectedNodeAttached: boolean;
  section: RoutingSection;
  labelPoint: C4ElkPoint;
  operationState?: Exclude<EmphasisState, "normal">;
  onOpenRelationship?: (relationshipId: string) => void;
}

interface C4LayoutEntry {
  node: DiagramNode;
  x: number;
  y: number;
  width: number;
  height: number;
  expandedGroup?: boolean;
}

interface C4LayoutResult {
  nodes: C4LayoutEntry[];
  edgeSections: Map<string, RoutingSection>;
  edgeLabels: Map<string, RoutingLabel>;
}

interface C4ElkPoint {
  x: number;
  y: number;
}

interface C4NodeDimensions {
  width: number;
  height: number;
}

interface C4LabelDimensions {
  width: number;
  height: number;
}

type C4SpatialDirection = "left" | "right" | "down" | "up";

interface C4SpatialNodePosition {
  id: string;
  parentId?: string | null;
  x: number;
  y: number;
  width?: number;
  height?: number;
}

const ELEMENT_TYPE_LABELS: Record<CanvasNodeKind, string> = {
  person: "Person",
  softwareSystem: "System",
  container: "Container",
  dataStore: "Data Store",
  dataStoreCollection: "Table",
  component: "Component",
  codeElement: "Code",
};

const DATA_STORE_KIND_LABELS: Record<CanvasDataStoreKind, string> = {
  database: "Database",
  objectStore: "Object Store",
  bucket: "Bucket",
  artifactStore: "Artifact Store",
  fileStore: "File Store",
};

const TYPE_ORDER: Record<CanvasNodeKind, number> = {
  person: 0,
  softwareSystem: 1,
  container: 2,
  dataStore: 3,
  dataStoreCollection: 4,
  component: 5,
  codeElement: 6,
};

function softwareMapNodeTypeLabel(
  node: Pick<
    DiagramNode,
    "type" | "dataStoreKind" | "dataStoreSchemaSections"
  >,
) {
  if (node.type === "dataStore") {
    return DATA_STORE_KIND_LABELS[node.dataStoreKind ?? "database"];
  }
  if (node.type === "dataStoreCollection") {
    const sectionKind = node.dataStoreSchemaSections?.[0]?.kind;
    return sectionKind === "document" ? "Document" : "Table";
  }
  return ELEMENT_TYPE_LABELS[node.type];
}

function softwareMapDataStoreShape(
  kind: CanvasDataStoreKind | undefined,
): CanvasDataStoreShape {
  if (kind === "bucket" || kind === "objectStore") return "bucket";
  if (kind === "artifactStore" || kind === "fileStore") return "folder";
  return "cylinder";
}

const C4_NODE_WIDTH = 280;
const C4_MIN_NODE_HEIGHT = 112;
const C4_FLOW_MIN_ZOOM = 0.03;
const C4_FLOW_MAX_ZOOM = 1.6;
const C4_SELECTED_NODE_FOCUS_PADDING = 0.16;
const C4_SELECTED_NODE_FOCUS_DURATION_MS = 140;
const C4_FIT_VIEW_PADDING = 0.18;
const C4_FIT_VIEW_DURATION_MS = 140;
const C4_NAV_NODE_REVEAL_PADDING_PX = 8;
const C4_NAV_NODE_REVEAL_DURATION_MS = 110;
const C4_DESCRIPTION_CHARS_PER_LINE = 42;
const C4_TITLE_CHARS_PER_LINE = 28;
const C4_EDGE_LABEL_MAX_WIDTH = 132;
const C4_EDGE_LABEL_HORIZONTAL_PADDING = 16;
const C4_EDGE_LABEL_VERTICAL_PADDING = 8;
const C4_EDGE_LABEL_CHARS_PER_LINE = 18;
const C4_EDGE_LABEL_LINE_HEIGHT = 15;
const C4_EDGE_LABEL_LABEL_GUTTER = 8;
const C4_EDGE_LABEL_NODE_GUTTER = 14;
const C4_EDGE_LABEL_CANDIDATE_STEP = 28;
const C4_EXPANDED_GROUP_LABEL_HEADER_HEIGHT = 70;
const C4_LOCAL_GROUP_PADDING = {
  top: C4_EXPANDED_GROUP_LABEL_HEADER_HEIGHT,
  right: 36,
  bottom: 36,
  left: 36,
} as const;
const C4_LOCAL_SIBLING_X_GAP = 96;
const C4_LOCAL_SIBLING_Y_GAP = 72;
const C4_LOCAL_ROW_CLUSTER_GAP = 24;

const c4NodeTypes = {
  softwareMapC4: SoftwareMapC4Node,
  softwareMapC4Group: SoftwareMapC4GroupNode,
};
const c4EdgeTypes = {
  softwareMapC4Edge: SoftwareMapC4Edge,
};
const C4HoveredNodeContext = createContext<string | null>(null);
const c4Elk = new ELK();

function findSpatialC4Node(
  selectedNodeId: string | null | undefined,
  positions: readonly C4SpatialNodePosition[],
  direction: C4SpatialDirection,
): string | null {
  const visiblePositions = positions.filter(
    (position) => Number.isFinite(position.x) && Number.isFinite(position.y),
  );
  const current = selectedNodeId
    ? visiblePositions.find((position) => position.id === selectedNodeId)
    : null;
  if (!current) {
    return firstC4SpatialNode(visiblePositions);
  }

  const scopedPositions = visiblePositions.filter(
    (position) => position.parentId === current.parentId,
  );
  const currentRect = c4SpatialRect(current);
  const sameLevelTarget = bestC4SpatialTarget({
    selectedNodeId: current.id,
    positions: scopedPositions,
    currentRect,
    direction,
  });
  if (sameLevelTarget) return sameLevelTarget;

  return bestC4SpatialTarget({
    selectedNodeId: current.id,
    positions: visiblePositions.filter(
      (position) => position.parentId === current.id,
    ),
    currentRect,
    direction,
  });
}

function bestC4SpatialTarget(input: {
  selectedNodeId: string;
  positions: readonly C4SpatialNodePosition[];
  currentRect: ReturnType<typeof c4SpatialRect>;
  direction: C4SpatialDirection;
}): string | null {
  let best: { id: string; score: number } | null = null;
  for (const position of input.positions) {
    if (position.id === input.selectedNodeId) continue;
    const score = c4SpatialScore(
      input.currentRect,
      c4SpatialRect(position),
      input.direction,
    );
    if (score === null) continue;
    if (!best || score < best.score) best = { id: position.id, score };
  }
  return best?.id ?? null;
}

function c4SpatialRect(position: C4SpatialNodePosition) {
  const width = position.width ?? 0;
  const height = position.height ?? 0;
  return {
    left: position.x,
    right: position.x + width,
    top: position.y,
    bottom: position.y + height,
    centerX: position.x + width / 2,
    centerY: position.y + height / 2,
  };
}

function c4SpatialScore(
  current: ReturnType<typeof c4SpatialRect>,
  candidate: ReturnType<typeof c4SpatialRect>,
  direction: C4SpatialDirection,
): number | null {
  if (direction === "left" && candidate.centerX >= current.centerX) return null;
  if (direction === "right" && candidate.centerX <= current.centerX)
    return null;
  if (direction === "up" && candidate.centerY >= current.centerY) return null;
  if (direction === "down" && candidate.centerY <= current.centerY) return null;

  const vertical = direction === "up" || direction === "down";
  const primaryGap =
    direction === "left"
      ? Math.max(0, current.left - candidate.right)
      : direction === "right"
        ? Math.max(0, candidate.left - current.right)
        : direction === "up"
          ? Math.max(0, current.top - candidate.bottom)
          : Math.max(0, candidate.top - current.bottom);
  const crossGap = vertical
    ? intervalGap(current.left, current.right, candidate.left, candidate.right)
    : intervalGap(current.top, current.bottom, candidate.top, candidate.bottom);
  const crossCenterDistance = vertical
    ? Math.abs(candidate.centerX - current.centerX)
    : Math.abs(candidate.centerY - current.centerY);
  if (crossGap === 0) return primaryGap * 1000 + crossCenterDistance;
  return 1_000_000_000 + crossGap * 1000 + primaryGap;
}

function intervalGap(
  leftStart: number,
  leftEnd: number,
  rightStart: number,
  rightEnd: number,
): number {
  if (rightEnd < leftStart) return leftStart - rightEnd;
  if (rightStart > leftEnd) return rightStart - leftEnd;
  return 0;
}

function firstC4SpatialNode(
  positions: readonly C4SpatialNodePosition[],
): string | null {
  return (
    [...positions].sort((left, right) => {
      const dy = left.y - right.y;
      if (dy !== 0) return dy;
      const dx = left.x - right.x;
      if (dx !== 0) return dx;
      return left.id.localeCompare(right.id);
    })[0]?.id ?? null
  );
}

function c4SpatialDirectionForKey(
  key: string,
): C4SpatialDirection | null {
  if (key === "h" || key === "ArrowLeft") return "left";
  if (key === "j" || key === "ArrowDown") return "down";
  if (key === "k" || key === "ArrowUp") return "up";
  if (key === "l" || key === "ArrowRight") return "right";
  return null;
}

function c4SpatialPositions(
  layout: C4LayoutResult | null,
): C4SpatialNodePosition[] {
  return (
    layout?.nodes.map((entry) => ({
      id: entry.node.id,
      parentId: entry.node.parentId ?? null,
      x: entry.x,
      y: entry.y,
      width: entry.width,
      height: entry.height,
    })) ?? []
  );
}

function c4DisplayedSnapshotForCurrentState(
  layoutSnapshot: DiagramSnapshot,
  currentSnapshot: DiagramSnapshot,
): DiagramSnapshot {
  const layoutNodes = layoutSnapshot.nodes;
  const layoutNodeIds = new Set(layoutNodes.map((node) => node.id));
  const currentSelectedNodeId =
    currentSnapshot.selectedNodeId &&
    layoutNodeIds.has(currentSnapshot.selectedNodeId)
      ? currentSnapshot.selectedNodeId
      : null;
  const layoutSelectedNodeId =
    layoutSnapshot.selectedNodeId &&
    layoutNodeIds.has(layoutSnapshot.selectedNodeId)
      ? layoutSnapshot.selectedNodeId
      : null;

  return {
    ...layoutSnapshot,
    selectedNodeId: currentSelectedNodeId ?? layoutSelectedNodeId,
  };
}

function softwareMapNodeIdForDrill(input: {
  node: Pick<DiagramNode, "id" | "expanded">;
  nodes: readonly Pick<DiagramNode, "id" | "parentId">[];
}): string {
  if (!input.node.expanded) return input.node.id;
  return (
    input.nodes.find((node) => node.parentId === input.node.id)?.id ??
    input.node.id
  );
}

function parentSoftwareMapNodeId(input: {
  nodes: readonly Pick<DiagramNode, "id" | "parentId">[];
  nodeId: string | null | undefined;
}): string | null {
  if (!input.nodeId) return null;
  const selected = input.nodes.find((node) => node.id === input.nodeId);
  if (!selected?.parentId) return null;
  return input.nodes.some((node) => node.id === selected.parentId)
    ? selected.parentId
    : null;
}

function softwareMapNodeForKeyboardExpansion<
  TNode extends Pick<DiagramNode, "id" | "expandable">,
>(input: {
  nodes: readonly TNode[];
  selectedNodeId: string | null | undefined;
  focusedNodeId?: string | null | undefined;
}): TNode | null {
  if (input.selectedNodeId) {
    const selected = input.nodes.find(
      (node) => node.id === input.selectedNodeId,
    );
    return selected?.expandable ? selected : null;
  }

  if (input.focusedNodeId) {
    const focused = input.nodes.find((node) => node.id === input.focusedNodeId);
    return focused?.expandable ? focused : null;
  }

  return null;
}

function softwareMapViewportFocusNodeId(input: {
  nodes: readonly Pick<DiagramNode, "id">[];
  viewportFocusNodeId: string | null | undefined;
}): string | null {
  const nodeIds = new Set(input.nodes.map((node) => node.id));
  if (input.viewportFocusNodeId && nodeIds.has(input.viewportFocusNodeId)) {
    return input.viewportFocusNodeId;
  }
  return null;
}

function softwareMapViewportFocusTargetReady(input: {
  node: Pick<DiagramNode, "id" | "expanded">;
  viewportFocusNodeId: string | null | undefined;
  requireExpanded?: boolean;
}) {
  if (input.viewportFocusNodeId !== input.node.id) return true;
  return input.requireExpanded === false || input.node.expanded;
}

const SOFTWARE_MAP_KEYBOARD_NODE_ID_ATTRIBUTE = "data-software-map-node-id";
const SOFTWARE_MAP_KEYBOARD_NODE_SELECTOR = `[${SOFTWARE_MAP_KEYBOARD_NODE_ID_ATTRIBUTE}]`;

function softwareMapKeyboardNodeDomAttributes(
  nodeId: string,
): C4MapAnyFlowNode["domAttributes"] {
  return {
    [SOFTWARE_MAP_KEYBOARD_NODE_ID_ATTRIBUTE]: nodeId,
  } as unknown as C4MapAnyFlowNode["domAttributes"];
}

function softwareMapEventTargetNodeId(
  target: EventTarget | null,
  currentTarget: HTMLElement,
): string | null {
  if (typeof HTMLElement === "undefined") return null;
  if (!(target instanceof HTMLElement)) return null;
  const nodeElement = target.closest<HTMLElement>(
    SOFTWARE_MAP_KEYBOARD_NODE_SELECTOR,
  );
  if (!nodeElement || !currentTarget.contains(nodeElement)) return null;
  return nodeElement.getAttribute(SOFTWARE_MAP_KEYBOARD_NODE_ID_ATTRIBUTE);
}

function isSoftwareMapEditableTarget(target: EventTarget | null) {
  if (typeof HTMLElement === "undefined") return false;
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable ||
      target.tagName === "INPUT" ||
      target.tagName === "TEXTAREA" ||
      target.tagName === "SELECT")
  );
}

function focusSoftwareMapKeyboardTarget(element: HTMLElement | null) {
  if (!element || typeof document === "undefined") return;
  const activeElement = document.activeElement;
  if (isSoftwareMapEditableTarget(activeElement)) return;
  if (activeElement === element) return;
  element.focus({ preventScroll: true });
}

export function DiagramCanvas({
  snapshot,
  height,
  onSelectNode,
  onExpandNode,
  onCollapseNode,
  onToggleNodeExpansion,
  onFocusNode,
  relationshipStateById,
  onOpenRelationship,
  viewportFocusNodeId,
  onViewportFocusComplete,
}: {
  snapshot: DiagramSnapshot;
  height?: number | string;
  onSelectNode?: (node: DiagramNode) => void;
  onExpandNode?: (node: DiagramNode) => void;
  onCollapseNode?: (node: DiagramNode) => void;
  onToggleNodeExpansion?: (node: DiagramNode) => void;
  onFocusNode?: (node: DiagramNode) => void;
  relationshipStateById?: ReadonlyMap<
    string,
    Exclude<EmphasisState, "normal">
  >;
  onOpenRelationship?: (relationshipId: string) => void;
  viewportFocusNodeId?: string | null;
  onViewportFocusComplete?: (nodeId: string) => void;
}) {
  const style = height
    ? ({
        "--software-map-height": typeof height === "number" ? `${height}px` : height,
      } as CSSProperties)
    : undefined;
  return (
    <figure className="software-map-frame software-map-frame--chrome-hidden" style={style}>
      <div className="software-map-body">
        <div className="software-map-canvas">
          <C4MapCanvas
            snapshot={snapshot}
            onSelectNode={onSelectNode}
            onExpandNode={onExpandNode}
            onCollapseNode={onCollapseNode}
            onToggleNodeExpansion={onToggleNodeExpansion}
            onFocusNode={onFocusNode}
            relationshipStateById={relationshipStateById}
            onOpenRelationship={onOpenRelationship}
            viewportFocusNodeId={viewportFocusNodeId}
            onViewportFocusComplete={onViewportFocusComplete}
          />
        </div>
      </div>
    </figure>
  );
}

function C4MapCanvas({
  snapshot,
  onSelectNode,
  onExpandNode,
  onCollapseNode,
  onToggleNodeExpansion,
  onFocusNode,
  relationshipStateById,
  onOpenRelationship,
  viewportFocusNodeId,
  onViewportFocusComplete,
}: {
  snapshot: DiagramSnapshot;
  onSelectNode?: (node: DiagramNode) => void;
  onExpandNode?: (node: DiagramNode) => void;
  onCollapseNode?: (node: DiagramNode) => void;
  onToggleNodeExpansion?: (node: DiagramNode) => void;
  onFocusNode?: (node: DiagramNode) => void;
  relationshipStateById?: ReadonlyMap<
    string,
    Exclude<EmphasisState, "normal">
  >;
  onOpenRelationship?: (relationshipId: string) => void;
  viewportFocusNodeId?: string | null;
  onViewportFocusComplete?: (nodeId: string) => void;
}) {
  const [layoutState, setLayoutState] = useState<C4DisplayedLayoutState | null>(
    null,
  );
  const [layoutError, setLayoutError] = useState<string | null>(null);
  const keyboardTargetRef = useRef<HTMLDivElement | null>(null);
  const [flowInstance, setFlowInstance] = useState<ReactFlowInstance<
    C4MapAnyFlowNode,
    ReactFlowEdge
  > | null>(null);
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);
  const flowRef = useRef<ReactFlowInstance<
    C4MapAnyFlowNode,
    ReactFlowEdge
  > | null>(null);
  const previousInlineLayoutRef = useRef<{
    layout: InlineC4LayoutResult;
    relationships: readonly DiagramRelationship[];
  } | null>(null);
  const appliedLayoutSignatureRef = useRef<string | null>(null);
  const [nodeMeasurement, setNodeMeasurement] = useState<{
    key: string;
    dimensions: ReadonlyMap<string, C4NodeDimensions>;
  } | null>(null);
  const measuredNodes = snapshot.nodes;
  const measuredRelationships = snapshot.relationships;
  const displayedSnapshot = useMemo(
    () =>
      layoutState
        ? c4DisplayedSnapshotForCurrentState(layoutState.snapshot, snapshot)
        : snapshot,
    [layoutState, snapshot],
  );
  const layout = layoutState?.layout ?? null;
  const nodes = displayedSnapshot.nodes;
  const theme = "dark" as const;
  const measurementKey = useMemo(
    () => c4MeasurementKey(measuredNodes),
    [measuredNodes],
  );
  const nodeDimensions =
    nodeMeasurement?.key === measurementKey ? nodeMeasurement.dimensions : null;
  const hasMeasuredNodes =
    measuredNodes.length === 0 ||
    (nodeDimensions !== null &&
      measuredNodes.every((node) => nodeDimensions.has(node.id)));

  const handleMeasuredNodes = useCallback(
    (nextDimensions: ReadonlyMap<string, C4NodeDimensions>) => {
      setNodeMeasurement((currentMeasurement) =>
        currentMeasurement?.key === measurementKey &&
        c4DimensionsEqual(currentMeasurement.dimensions, nextDimensions)
          ? currentMeasurement
          : { key: measurementKey, dimensions: nextDimensions },
      );
    },
    [measurementKey],
  );
  const layoutSignature = useMemo(
    () =>
      hasMeasuredNodes
        ? c4LayoutSignature(
            measuredNodes,
            measuredRelationships,
            nodeDimensions,
          )
        : "",
    [hasMeasuredNodes, measuredNodes, measuredRelationships, nodeDimensions],
  );
  const layoutInputRef = useRef({
    snapshot,
    nodeDimensions,
  });
  layoutInputRef.current = {
    snapshot,
    nodeDimensions,
  };

  useEffect(() => {
    if (!hasMeasuredNodes || !layoutSignature) return;
    if (appliedLayoutSignatureRef.current === layoutSignature) return;
    let cancelled = false;
    setLayoutError(null);
    const {
      nodeDimensions: layoutNodeDimensions,
      snapshot: layoutSnapshot,
    } = layoutInputRef.current;
    const {
      nodes: layoutNodes,
      relationships: layoutRelationships,
    } = layoutSnapshot;
    const previousInlineLayout = c4PreviousInlineLayoutForRelationships({
      previousLayout: previousInlineLayoutRef.current?.layout,
      previousRelationships: previousInlineLayoutRef.current?.relationships,
      currentRelationships: layoutRelationships,
    });
    void runInlineC4Layout(
      layoutNodes,
      layoutRelationships,
      layoutNodeDimensions ?? undefined,
      // A newly resolved edge changes the graph that determines node
      // placement. Reusing a no-edge layout keeps the graph in its old
      // stack, even though the edge itself is present.
      previousInlineLayout,
      (globalThis as { __CODE_DIAGRAM_LIBAVOID_WASM_URL__?: string })
        .__CODE_DIAGRAM_LIBAVOID_WASM_URL__,
    )
      .then((nextLayout) => {
        if (cancelled || !nextLayout) return;
        appliedLayoutSignatureRef.current = layoutSignature;
        previousInlineLayoutRef.current = {
          layout: nextLayout.inlineLayout,
          relationships: layoutRelationships,
        };
        setLayoutState({
          signature: layoutSignature,
          snapshot: layoutSnapshot,
          layout: nextLayout.layout,
        });
      })
      .catch((caught: unknown) => {
        if (cancelled) return;
        setLayoutError(
          caught instanceof Error ? caught.message : String(caught),
        );
      });
    return () => {
      cancelled = true;
    };
  }, [hasMeasuredNodes, layoutSignature]);
  const layoutRefreshing = Boolean(
    layoutState && layoutSignature && layoutState.signature !== layoutSignature,
  );

  const drillNode = useCallback(
    (node: DiagramNode) => {
      const drillNodeId = softwareMapNodeIdForDrill({ node, nodes });
      if (drillNodeId !== node.id) {
        const childNode = nodes.find(
          (candidate) => candidate.id === drillNodeId,
        );
        if (childNode) onSelectNode?.(childNode);
        return;
      }
      onExpandNode?.(node);
    },
    [nodes, onExpandNode, onSelectNode],
  );

  const flow = useMemo(
    () =>
      layout
        ? createC4MapFlowFromLayout(displayedSnapshot, layout, {
            onSelectNode,
            onExpandNode,
            onCollapseNode,
            nodeDimensions: nodeDimensions ?? undefined,
            relationshipStateById,
            onOpenRelationship,
          })
        : null,
    [
      layout,
      nodeDimensions,
      onCollapseNode,
      onExpandNode,
      onSelectNode,
      onOpenRelationship,
      relationshipStateById,
      displayedSnapshot,
    ],
  );
  useEffect(() => {
    if (!flowInstance || !layout) return;
    const canvas = keyboardTargetRef.current;
    let frame = 0;
    const scheduleFit = () => {
      if (frame !== 0) cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        frame = 0;
        fitC4MapView(flowRef.current);
      });
    };
    scheduleFit();
    if (!canvas || typeof ResizeObserver !== "function") {
      return () => {
        if (frame !== 0) cancelAnimationFrame(frame);
      };
    }
    const resizeObserver = new ResizeObserver(scheduleFit);
    resizeObserver.observe(canvas);
    return () => {
      if (frame !== 0) cancelAnimationFrame(frame);
      resizeObserver.disconnect();
    };
  }, [flowInstance, layout]);

  useEffect(() => {
    const focusNodeId = softwareMapViewportFocusNodeId({
      nodes: flow?.nodes ?? [],
      viewportFocusNodeId,
    });
    const focused = focusNodeId
      ? flow?.nodes.find((node) => node.id === focusNodeId)
      : null;
    if (!focused) return;
    if (
      !softwareMapViewportFocusTargetReady({
        node: focused.data.node,
        viewportFocusNodeId,
      })
    ) {
      return;
    }
    const frame = requestAnimationFrame(() => {
      if (!focusC4MapNode(flowRef.current, focused)) return;
      focusSoftwareMapKeyboardTarget(keyboardTargetRef.current);
      if (viewportFocusNodeId === focused.id) {
        onViewportFocusComplete?.(focused.id);
      }
    });
    return () => cancelAnimationFrame(frame);
  }, [
    flowInstance,
    flow?.nodes,
    onViewportFocusComplete,
    viewportFocusNodeId,
  ]);

  useLayoutEffect(() => {
    if (!displayedSnapshot.selectedNodeId) return;
    focusSoftwareMapKeyboardTarget(keyboardTargetRef.current);
  }, [displayedSnapshot.selectedNodeId, displayedSnapshot.view]);

  const handleKeyDown = useCallback(
    (event: ReactKeyboardEvent<HTMLDivElement>) => {
      if (event.defaultPrevented) return;
      if (
        isSoftwareMapEditableTarget(event.target) ||
        event.metaKey ||
        event.ctrlKey ||
        event.altKey
      ) {
        return;
      }

      if (event.key.toLowerCase() === "f") {
        event.preventDefault();
        event.stopPropagation();
        fitC4MapView(flowRef.current);
        return;
      }

      const direction = c4SpatialDirectionForKey(event.key);
      if (direction) {
        event.preventDefault();
        event.stopPropagation();
        const nextId = findSpatialC4Node(
          displayedSnapshot.selectedNodeId,
          c4SpatialPositions(layout),
          direction,
        );
        const nextNode = nextId
          ? nodes.find((candidate) => candidate.id === nextId)
          : null;
        if (nextNode) {
          onSelectNode?.(nextNode);
          const flowNode = flow?.nodes.find((node) => node.id === nextNode.id);
          if (flowNode) {
            revealC4MapNode(
              flowRef.current,
              keyboardTargetRef.current,
              flowNode,
            );
          }
        }
        return;
      }

      if (event.key === "Enter") {
        const selected = displayedSnapshot.selectedNodeId
          ? nodes.find((node) => node.id === displayedSnapshot.selectedNodeId)
          : null;
        if (selected) {
          event.preventDefault();
          event.stopPropagation();
          drillNode(selected);
        }
        return;
      }

      if (event.key === "Tab") {
        const selected = softwareMapNodeForKeyboardExpansion({
          nodes,
          selectedNodeId: displayedSnapshot.selectedNodeId,
          focusedNodeId: softwareMapEventTargetNodeId(
            event.target,
            event.currentTarget,
          ),
        });
        if (selected) {
          event.preventDefault();
          event.stopPropagation();
          onToggleNodeExpansion?.(selected);
        }
        return;
      }

      if (event.key === "Escape") {
        const parentId = parentSoftwareMapNodeId({
          nodes,
          nodeId: displayedSnapshot.selectedNodeId,
        });
        const parent = parentId
          ? nodes.find((node) => node.id === parentId)
          : null;
        if (parent) {
          event.preventDefault();
          event.stopPropagation();
          onSelectNode?.(parent);
          onFocusNode?.(parent);
        }
      }
    },
    [
      layout,
      flow?.nodes,
      drillNode,
      nodes,
      onFocusNode,
      onSelectNode,
      onToggleNodeExpansion,
      displayedSnapshot.selectedNodeId,
    ],
  );
  const handleKeyDownCapture = useCallback(
    (event: ReactKeyboardEvent<HTMLDivElement>) => {
      if (event.key === "Tab") {
        handleKeyDown(event);
      }
    },
    [handleKeyDown],
  );

  return (
    <div
      ref={keyboardTargetRef}
      className="software-map-c4-canvas"
      tabIndex={0}
      onKeyDownCapture={handleKeyDownCapture}
      onKeyDown={handleKeyDown}
    >
      <C4NodeMeasurementLayer
        nodes={measuredNodes}
        measurementKey={measurementKey}
        onMeasure={handleMeasuredNodes}
      />
      {layoutError ? (
        <div className="software-map-code-status">
          Layout failed: {layoutError}
        </div>
      ) : (
        <>
          {layoutRefreshing ? (
            <div className="software-map-code-status">Refreshing layout...</div>
          ) : null}
          {flow ? (
            <>
              <C4HoveredNodeContext.Provider value={hoveredNodeId}>
                <ReactFlow
                  colorMode={theme}
                  proOptions={{ hideAttribution: true }}
                  nodes={flow.nodes}
                  edges={flow.edges}
                  nodeTypes={c4NodeTypes}
                  edgeTypes={c4EdgeTypes}
                  nodesDraggable={false}
                  nodesConnectable={false}
                  elementsSelectable
                  panActivationKeyCode={null}
                  fitView
                  fitViewOptions={{ padding: C4_FIT_VIEW_PADDING }}
                  minZoom={C4_FLOW_MIN_ZOOM}
                  maxZoom={C4_FLOW_MAX_ZOOM}
                  panOnScroll={false}
                  preventScrolling={false}
                  zoomOnScroll={false}
                  zoomOnPinch={false}
                  zoomOnDoubleClick={false}
                  onInit={(instance) => {
                    flowRef.current = instance;
                    setFlowInstance(instance);
                  }}
                  onNodeClick={(_, node) => onSelectNode?.(node.data.node)}
                  onNodeMouseEnter={(_, node) => setHoveredNodeId(node.id)}
                  onNodeMouseLeave={(_, node) =>
                    setHoveredNodeId((currentNodeId) =>
                      currentNodeId === node.id ? null : currentNodeId,
                    )
                  }
                >
                </ReactFlow>
              </C4HoveredNodeContext.Provider>
            </>
          ) : (
            <div className="software-map-code-status">
              Laying out software map...
            </div>
          )}
        </>
      )}
    </div>
  );
}

async function runInlineC4Layout(
  nodes: DiagramNode[],
  relationships: DiagramRelationship[],
  nodeDimensions?: ReadonlyMap<string, C4NodeDimensions>,
  previousLayout?: InlineC4LayoutResult,
  wasmUrl?: string,
): Promise<{ layout: C4LayoutResult; inlineLayout: InlineC4LayoutResult }> {
  const layout = await runC4LocalInflateLayout(
    nodes,
    relationships,
    nodeDimensions,
    previousLayout ?? c4EmptyInlineLayout(),
    wasmUrl,
  );
  return {
    inlineLayout: inlineLayoutFromC4Layout(layout),
    layout,
  };
}

function c4EmptyInlineLayout(): InlineC4LayoutResult {
  return {
    nodeBboxes: new Map(),
    groupBboxes: new Map(),
  };
}

function inlineLayoutFromC4Layout(
  layout: C4LayoutResult,
): InlineC4LayoutResult {
  const nodeBboxes = new Map<string, C4LayoutBox>();
  const groupBboxes = new Map<string, C4LayoutBox>();
  for (const entry of layout.nodes) {
    (entry.expandedGroup ? groupBboxes : nodeBboxes).set(entry.node.id, {
      x: entry.x,
      y: entry.y,
      width: entry.width,
      height: entry.height,
    });
  }
  return { nodeBboxes, groupBboxes };
}

function createC4MapFlowFromLayout(
  snapshot: DiagramSnapshot,
  layout: C4LayoutResult,
  options: {
    onSelectNode?: (node: DiagramNode) => void;
    onExpandNode?: (node: DiagramNode) => void;
    onCollapseNode?: (node: DiagramNode) => void;
    nodeDimensions?: ReadonlyMap<string, C4NodeDimensions> | null;
    relationshipStateById?: ReadonlyMap<
      string,
      Exclude<EmphasisState, "normal">
    >;
    onOpenRelationship?: (relationshipId: string) => void;
  } = {},
): { nodes: C4MapAnyFlowNode[]; edges: ReactFlowEdge[] } {
  const flowNodes = layout.nodes.map(
    ({ node, x, y, width, height, expandedGroup }) => {
      const measured = options.nodeDimensions?.get(node.id);
      const renderedWidth = Math.max(width, measured?.width ?? 0);
      const renderedHeight = Math.max(height, measured?.height ?? 0);
      const baseFlowNode = {
        id: node.id,
        position: { x, y },
        width: renderedWidth,
        height: renderedHeight,
        data: {
          node,
          selected: snapshot.selectedNodeId === node.id,
          onSelect: options.onSelectNode,
          onExpandNode: options.onExpandNode,
          onCollapseNode: options.onCollapseNode,
        },
        draggable: false,
        selectable: true,
        domAttributes: softwareMapKeyboardNodeDomAttributes(node.id),
        style: { width: renderedWidth, height: renderedHeight },
      };
      return expandedGroup
        ? {
            ...baseFlowNode,
            type: "softwareMapC4Group" as const,
            zIndex: 0,
          }
        : {
            ...baseFlowNode,
            type: "softwareMapC4" as const,
            zIndex: 2,
          };
    },
  );
  const nodeMetadataById = new Map(
    flowNodes.map((node) => [
      node.id,
      {
        type: node.data.node.type,
        bounds: {
          x: node.position.x,
          y: node.position.y,
          width: node.width,
          height: node.height,
        },
      },
    ]),
  );

  const flowEdges: ReactFlowEdge[] = snapshot.relationships.flatMap(
    (relationship) => {
      const sourceMetadata = nodeMetadataById.get(relationship.from);
      const targetMetadata = nodeMetadataById.get(relationship.to);
      if (!sourceMetadata || !targetMetadata) return [];
      const kind = relationship.kind;
      const semanticKind = relationship.semanticKind;
      const attachedToSelectedNode =
        snapshot.selectedNodeId === relationship.from ||
        snapshot.selectedNodeId === relationship.to;
      const operationState = options.relationshipStateById?.get(relationship.id);
      const operationHighlightState =
        operationState && operationState !== "inactive"
          ? operationState
          : undefined;
      const operationActive = operationState === "active";
      const color = attachedToSelectedNode
        ? "var(--accent)"
        : operationActive
          ? "var(--selection)"
          : c4EdgeColor(kind, semanticKind);
      const label = relationship.hideLabel
        ? undefined
        : (relationship.label ?? relationship.semanticKind);
      const section = layout.edgeSections.get(relationship.id);
      if (!section) return [];
      const labelDimensions = label
        ? estimateC4EdgeLabelDimensions(label)
        : undefined;
      const labelFallbackPoints = edgePointsFromSection(section);
      const handles = c4EdgeHandles(
        sourceMetadata.bounds,
        targetMetadata.bounds,
        section,
      );
      const strokeDasharray = c4EdgeDasharray(
        kind,
        sourceMetadata.type,
        targetMetadata.type,
        semanticKind,
      );
      return [
        {
          id: relationship.id,
          source: relationship.from,
          target: relationship.to,
          sourceHandle: handles.sourceHandle,
          targetHandle: handles.targetHandle,
          type: "softwareMapC4Edge",
          markerEnd:
            semanticKind === "forbidden"
              ? undefined
              : {
                  type:
                    semanticKind === "async"
                      ? MarkerType.Arrow
                      : MarkerType.ArrowClosed,
                  color,
                },
          label,
          className: [
            "software-map-c4-edge",
            `software-map-c4-edge--${kind}`,
            isCanvasRelationshipSemanticKind(semanticKind)
              ? `software-map-c4-edge--semantic-${semanticKind}`
              : "",
            attachedToSelectedNode ? "software-map-c4-edge--selected-node" : "",
            operationHighlightState
              ? `software-map-c4-edge--operation-${operationHighlightState}`
              : "",
          ]
            .filter(Boolean)
            .join(" "),
          zIndex: operationActive ? 4 : attachedToSelectedNode ? 3 : 1,
          style: {
            stroke: color,
            strokeWidth: operationActive ? 3 : attachedToSelectedNode ? 2.5 : 2,
            strokeDasharray,
            strokeLinecap: strokeDasharray ? "round" : undefined,
          },
          data: {
            relationship,
            selectedNodeAttached: attachedToSelectedNode,
            section,
            labelPoint: c4EdgeLabelPoint(
              label ? layout.edgeLabels.get(relationship.id) : undefined,
              labelDimensions,
              labelFallbackPoints,
            ),
            operationState,
            onOpenRelationship: options.onOpenRelationship,
          },
          interactionWidth: 18,
        },
      ];
    },
  );
  return { nodes: flowNodes, edges: flowEdges };
}

function focusC4MapNode(
  flow: ReactFlowInstance<C4MapAnyFlowNode, ReactFlowEdge> | null,
  node: C4MapAnyFlowNode,
) {
  if (!flow) return false;
  const bounds = {
    x: node.position.x,
    y: node.position.y,
    width: c4FlowNodeWidth(node),
    height: c4FlowNodeHeight(node),
  };
  void flow.fitBounds(bounds, {
    padding: C4_SELECTED_NODE_FOCUS_PADDING,
    duration: C4_SELECTED_NODE_FOCUS_DURATION_MS,
  });
  return true;
}

function fitC4MapView(
  flow: Pick<
    ReactFlowInstance<C4MapAnyFlowNode, ReactFlowEdge>,
    "fitView"
  > | null,
) {
  if (!flow) return false;
  void flow.fitView({
    padding: C4_FIT_VIEW_PADDING,
    duration: C4_FIT_VIEW_DURATION_MS,
  });
  return true;
}

function revealC4MapNode(
  flow: ReactFlowInstance<C4MapAnyFlowNode, ReactFlowEdge> | null,
  viewportElement: HTMLElement | null,
  node: C4MapAnyFlowNode,
) {
  if (!flow || !viewportElement) return false;
  const nextViewport = c4ViewportForNodeReveal({
    nodeBounds: {
      x: node.position.x,
      y: node.position.y,
      width: c4FlowNodeWidth(node),
      height: c4FlowNodeHeight(node),
    },
    viewport: flow.getViewport(),
    viewportSize: {
      width: viewportElement.clientWidth,
      height: viewportElement.clientHeight,
    },
    padding: C4_NAV_NODE_REVEAL_PADDING_PX,
    minZoom: C4_FLOW_MIN_ZOOM,
    maxZoom: C4_FLOW_MAX_ZOOM,
  });
  if (!nextViewport) return false;
  void flow.setViewport(nextViewport, {
    duration: C4_NAV_NODE_REVEAL_DURATION_MS,
  });
  return true;
}

function c4ViewportForNodeReveal(input: {
  nodeBounds: { x: number; y: number; width: number; height: number };
  viewport: Viewport;
  viewportSize: { width: number; height: number };
  padding?: number;
  minZoom?: number;
  maxZoom?: number;
}): Viewport | null {
  const padding = Math.max(0, input.padding ?? 0);
  const minZoom = input.minZoom ?? C4_FLOW_MIN_ZOOM;
  const maxZoom = input.maxZoom ?? C4_FLOW_MAX_ZOOM;
  const { nodeBounds, viewport, viewportSize } = input;
  if (
    !c4FinitePositive(viewport.zoom) ||
    !c4FinitePositive(viewportSize.width) ||
    !c4FinitePositive(viewportSize.height) ||
    !c4FinitePositive(nodeBounds.width) ||
    !c4FinitePositive(nodeBounds.height)
  ) {
    return null;
  }

  const availableWidth = Math.max(1, viewportSize.width - padding * 2);
  const availableHeight = Math.max(1, viewportSize.height - padding * 2);
  const targetZoom = Math.max(
    minZoom,
    Math.min(
      maxZoom,
      viewport.zoom,
      availableWidth / nodeBounds.width,
      availableHeight / nodeBounds.height,
    ),
  );
  const currentCenter = {
    x: (viewportSize.width / 2 - viewport.x) / viewport.zoom,
    y: (viewportSize.height / 2 - viewport.y) / viewport.zoom,
  };
  const next = {
    x: viewportSize.width / 2 - currentCenter.x * targetZoom,
    y: viewportSize.height / 2 - currentCenter.y * targetZoom,
    zoom: targetZoom,
  };

  c4RevealAxis({
    next,
    axis: "x",
    nodeStart: nodeBounds.x,
    nodeSize: nodeBounds.width,
    viewportSize: viewportSize.width,
    padding,
  });
  c4RevealAxis({
    next,
    axis: "y",
    nodeStart: nodeBounds.y,
    nodeSize: nodeBounds.height,
    viewportSize: viewportSize.height,
    padding,
  });

  if (
    Math.abs(next.x - viewport.x) < 0.5 &&
    Math.abs(next.y - viewport.y) < 0.5 &&
    Math.abs(next.zoom - viewport.zoom) < 0.001
  ) {
    return null;
  }
  return next;
}

function c4RevealAxis(input: {
  next: Viewport;
  axis: "x" | "y";
  nodeStart: number;
  nodeSize: number;
  viewportSize: number;
  padding: number;
}) {
  const screenStart =
    input.nodeStart * input.next.zoom + input.next[input.axis];
  const screenEnd =
    (input.nodeStart + input.nodeSize) * input.next.zoom +
    input.next[input.axis];
  const visibleStart = input.padding;
  const visibleEnd = input.viewportSize - input.padding;

  if (screenEnd - screenStart > visibleEnd - visibleStart) {
    input.next[input.axis] =
      input.viewportSize / 2 -
      (input.nodeStart + input.nodeSize / 2) * input.next.zoom;
  } else if (screenStart < visibleStart) {
    input.next[input.axis] += visibleStart - screenStart;
  } else if (screenEnd > visibleEnd) {
    input.next[input.axis] -= screenEnd - visibleEnd;
  }
}

function c4FinitePositive(value: number) {
  return Number.isFinite(value) && value > 0;
}

function c4FlowNodeWidth(node: C4MapAnyFlowNode): number {
  return (
    numericStyleDimension(node.style?.width) ??
    (typeof node.width === "number"
      ? node.width
      : typeof node.measured?.width === "number"
        ? node.measured.width
        : C4_NODE_WIDTH)
  );
}

function c4FlowNodeHeight(node: C4MapAnyFlowNode): number {
  return (
    numericStyleDimension(node.style?.height) ??
    (typeof node.height === "number"
      ? node.height
      : typeof node.measured?.height === "number"
        ? node.measured.height
        : C4_MIN_NODE_HEIGHT)
  );
}

function numericStyleDimension(value: unknown): number | undefined {
  if (typeof value === "number") return value;
  if (typeof value === "string") {
    const parsed = Number.parseFloat(value);
    return Number.isFinite(parsed) ? parsed : undefined;
  }
  return undefined;
}

function c4PreviousLayoutGeometry(
  previousLayout?: InlineC4LayoutResult,
): {
  centers: Map<string, C4ElkPoint>;
  boxes: Map<string, C4LayoutBox>;
} {
  const centers = new Map<string, C4ElkPoint>();
  const boxes = new Map<string, C4LayoutBox>();
  if (!previousLayout) return { centers, boxes };
  // groupBboxes second so an expanded node's outer footprint wins.
  for (const bboxes of [
    previousLayout.nodeBboxes,
    previousLayout.groupBboxes,
  ]) {
    for (const [id, box] of bboxes) {
      boxes.set(id, box);
      centers.set(id, {
        x: box.x + box.width / 2,
        y: box.y + box.height / 2,
      });
    }
  }
  return { centers, boxes };
}

function compareC4NodesForLayout(
  left: DiagramNode,
  right: DiagramNode,
  previousCenters: ReadonlyMap<string, C4ElkPoint>,
  axis: C4LayoutAxis,
) {
  const leftCenter = previousCenters.get(left.id);
  const rightCenter = previousCenters.get(right.id);
  if (leftCenter && rightCenter) {
    const crossAxis: C4LayoutAxis =
      axis === "horizontal" ? "vertical" : "horizontal";
    return (
      c4PointAxisCoordinate(leftCenter, axis) -
        c4PointAxisCoordinate(rightCenter, axis) ||
      c4PointAxisCoordinate(leftCenter, crossAxis) -
        c4PointAxisCoordinate(rightCenter, crossAxis) ||
      left.label.localeCompare(right.label)
    );
  }
  if (leftCenter || rightCenter) return leftCenter ? -1 : 1;
  return (
    TYPE_ORDER[left.type] - TYPE_ORDER[right.type] ||
    left.label.localeCompare(right.label)
  );
}

function c4PreviousProxyCenter(
  nodeId: string,
  nodesById: ReadonlyMap<string, DiagramNode>,
  previousCenters: ReadonlyMap<string, C4ElkPoint>,
): C4ElkPoint | null {
  // Newly revealed children have no previous position; fall back to the
  // closest ancestor that does (e.g. the group that just expanded).
  let currentId: string | undefined | null = nodeId;
  while (currentId) {
    const center = previousCenters.get(currentId);
    if (center) return center;
    currentId = nodesById.get(currentId)?.parentId;
  }
  return null;
}

interface C4LocalInflateContext {
  nodes: DiagramNode[];
  nodesById: ReadonlyMap<string, DiagramNode>;
  childIdsByParentId: ReadonlyMap<string, readonly string[]>;
  relationships: DiagramRelationship[];
  nodeDimensions?: ReadonlyMap<string, C4NodeDimensions>;
  previousCenters: ReadonlyMap<string, C4ElkPoint>;
  previousBoxes: ReadonlyMap<string, C4LayoutBox>;
  previousExpandedNodeIds: ReadonlySet<string>;
}

interface C4LocalLayoutResult {
  entries: C4LayoutEntry[];
  bbox: C4LayoutBox;
}

interface C4LocalLayoutUnit {
  node: DiagramNode;
  seed: C4ElkPoint;
  width: number;
  height: number;
  rowGroupingHeight: number;
  previousBox?: C4LayoutBox;
  childLayout?: C4LocalLayoutResult;
}

async function runC4LocalInflateLayout(
  nodes: DiagramNode[],
  relationships: DiagramRelationship[],
  nodeDimensions: ReadonlyMap<string, C4NodeDimensions> | undefined,
  previousLayout: InlineC4LayoutResult,
  wasmUrl?: string,
): Promise<C4LayoutResult> {
  const nodesById = new Map(nodes.map((node) => [node.id, node]));
  const childIdsByParentId = new Map<string, string[]>();
  for (const node of nodes) {
    if (!node.parentId || !nodesById.has(node.parentId)) continue;
    const children = childIdsByParentId.get(node.parentId) ?? [];
    children.push(node.id);
    childIdsByParentId.set(node.parentId, children);
  }
  const previousGeometry = c4PreviousLayoutGeometry(previousLayout);

  const layout = await layoutC4LocalInflateLevel(null, {
    nodes,
    nodesById,
    childIdsByParentId,
    relationships,
    nodeDimensions,
    previousCenters: previousGeometry.centers,
    previousBoxes: previousGeometry.boxes,
    previousExpandedNodeIds: new Set(previousLayout.groupBboxes.keys()),
  });

  return routeC4FixedLayoutEdges(layout.entries, relationships, wasmUrl);
}

async function layoutC4LocalInflateLevel(
  parentId: string | null,
  context: C4LocalInflateContext,
): Promise<C4LocalLayoutResult> {
  const childIds = c4LocalVisibleChildIds(parentId, context);
  if (childIds.length === 0) return c4EmptyLocalLayout();

  const isolatedLayout = await c4LocalIsolatedLayout(
    parentId,
    childIds,
    context,
  );
  if (
    parentId &&
    isolatedLayout &&
    childIds.every((childId) => !context.previousCenters.has(childId)) &&
    childIds.every((childId) => {
      const child = context.nodesById.get(childId);
      return (
        !child?.expanded ||
        (context.childIdsByParentId.get(childId)?.length ?? 0) === 0
      );
    })
  ) {
    return isolatedLayout;
  }
  const fallbackCenters = c4CentersFromLayoutEntries(
    isolatedLayout?.entries ?? [],
  );
  const units: C4LocalLayoutUnit[] = [];
  for (const childId of childIds) {
    const node = context.nodesById.get(childId)!;
    const childLayout =
      node.expanded &&
      (context.childIdsByParentId.get(node.id)?.length ?? 0) > 0
        ? await layoutC4LocalInflateLevel(node.id, context)
        : undefined;
    const seed =
      context.previousCenters.get(node.id) ??
      fallbackCenters.get(node.id) ??
      c4LocalFallbackPoint(units.length);
    const dimensions = c4MeasuredNodeDimensions(node, context.nodeDimensions);
    const width = childLayout
      ? Math.max(
          dimensions.width +
            C4_LOCAL_GROUP_PADDING.left +
            C4_LOCAL_GROUP_PADDING.right,
          childLayout.bbox.width +
            C4_LOCAL_GROUP_PADDING.left +
            C4_LOCAL_GROUP_PADDING.right,
        )
      : dimensions.width;
    const height = childLayout
      ? Math.max(
          dimensions.height +
            C4_LOCAL_GROUP_PADDING.top +
            C4_LOCAL_GROUP_PADDING.bottom,
          childLayout.bbox.height +
            C4_LOCAL_GROUP_PADDING.top +
            C4_LOCAL_GROUP_PADDING.bottom,
        )
      : dimensions.height;
    const previousBox = context.previousBoxes.get(node.id);
    units.push({
      node,
      seed,
      width,
      height,
      previousBox,
      rowGroupingHeight: Math.min(previousBox?.height ?? height, height),
      childLayout,
    });
  }

  const placements = packC4LocalInflateUnits(
    units,
    c4LocalInflateAnchorId(units, context.previousExpandedNodeIds),
  );
  const entries = units.flatMap((unit) => {
    const placement = placements.get(unit.node.id)!;
    if (!unit.childLayout) {
      return [
        {
          node: unit.node,
          x: placement.x,
          y: placement.y,
          width: unit.width,
          height: unit.height,
        },
      ];
    }

    const childTarget = {
      x: placement.x + C4_LOCAL_GROUP_PADDING.left,
      y: placement.y + C4_LOCAL_GROUP_PADDING.top,
    };
    const childOffset = {
      x: childTarget.x - unit.childLayout.bbox.x,
      y: childTarget.y - unit.childLayout.bbox.y,
    };
    return [
      {
        node: unit.node,
        x: placement.x,
        y: placement.y,
        width: unit.width,
        height: unit.height,
        expandedGroup: true,
      },
      ...unit.childLayout.entries.map((entry) => ({
        ...entry,
        x: entry.x + childOffset.x,
        y: entry.y + childOffset.y,
      })),
    ];
  });

  return { entries, bbox: c4LayoutEntriesBbox(entries) };
}

function c4LocalVisibleChildIds(
  parentId: string | null,
  context: C4LocalInflateContext,
): string[] {
  if (parentId) return [...(context.childIdsByParentId.get(parentId) ?? [])];
  return context.nodes
    .filter((node) => {
      if (!node.parentId) return true;
      const parent = context.nodesById.get(node.parentId);
      return !parent?.expanded;
    })
    .map((node) => node.id);
}

async function c4LocalIsolatedLayout(
  parentId: string | null,
  childIds: readonly string[],
  context: C4LocalInflateContext,
): Promise<C4LocalLayoutResult | null> {
  if (childIds.every((childId) => context.previousCenters.has(childId))) {
    return null;
  }

  const childNodes = childIds.map(
    (childId) => context.nodesById.get(childId)!,
  );
  const childRelationships = c4LocalProjectedRelationships(
    parentId,
    childIds,
    context,
  );
  const isolated = await runC4ElkLayout(
    childNodes,
    childRelationships,
    context.nodeDimensions,
    {
      axis: c4ChildLayoutAxis(
        parentId ? context.nodesById.get(parentId) : undefined,
      ),
    },
  );
  const isolatedBbox = c4LayoutEntriesBbox(isolated);
  const isolatedCenter = {
    x: isolatedBbox.x + isolatedBbox.width / 2,
    y: isolatedBbox.y + isolatedBbox.height / 2,
  };
  const parentCenter = parentId
    ? (context.previousCenters.get(parentId) ?? isolatedCenter)
    : isolatedCenter;
  const offset = {
    x: parentCenter.x - isolatedCenter.x,
    y: parentCenter.y - isolatedCenter.y,
  };
  const entries = isolated.map((entry) => ({
      ...entry,
      x: entry.x + offset.x,
      y: entry.y + offset.y,
    }));
  return { entries, bbox: c4LayoutEntriesBbox(entries) };
}

function c4LocalProjectedRelationships(
  parentId: string | null,
  childIds: readonly string[],
  context: C4LocalInflateContext,
): DiagramRelationship[] {
  const childIdSet = new Set(childIds);
  return context.relationships.flatMap((relationship) => {
    const from = c4LocalChildProxyId(
      relationship.from,
      childIdSet,
      context.nodesById,
    );
    const to = c4LocalChildProxyId(
      relationship.to,
      childIdSet,
      context.nodesById,
    );
    if (!from || !to || from === to) return [];
    // A proxied endpoint is an ancestor of the original node, so schema
    // endpoints (which name field rows on the original node) no longer apply.
    return [
      {
        ...relationship,
        id: `layout:${parentId ?? "root"}:${relationship.id}`,
        from,
        to,
        hideLabel: true,
        ...(from !== relationship.from
          ? {
              fromSchemaEndpointKind: undefined,
              fromSchemaFieldPath: undefined,
            }
          : {}),
        ...(to !== relationship.to
          ? {
              toSchemaEndpointKind: undefined,
              toSchemaFieldPath: undefined,
            }
          : {}),
      },
    ];
  });
}

function c4LocalChildProxyId(
  nodeId: string,
  childIds: ReadonlySet<string>,
  nodesById: ReadonlyMap<string, DiagramNode>,
): string | null {
  const visited = new Set<string>();
  let currentId: string | null | undefined = nodeId;
  while (currentId && !visited.has(currentId)) {
    if (childIds.has(currentId)) return currentId;
    visited.add(currentId);
    currentId = nodesById.get(currentId)?.parentId;
  }
  return null;
}

function c4CentersFromLayoutEntries(
  entries: readonly C4LayoutEntry[],
): Map<string, C4ElkPoint> {
  return new Map(
    entries.map((entry) => [
      entry.node.id,
      {
        x: entry.x + entry.width / 2,
        y: entry.y + entry.height / 2,
      },
    ]),
  );
}

function c4LocalInflateAnchorId(
  units: readonly C4LocalLayoutUnit[],
  previousExpandedNodeIds: ReadonlySet<string>,
): string | null {
  const toggledUnits = units
    .filter(
      (unit) =>
        Boolean(unit.node.expanded) !==
        previousExpandedNodeIds.has(unit.node.id),
    )
    .sort(c4LocalUnitSeedOrder);
  if (toggledUnits[0]) return toggledUnits[0].node.id;

  const resizedUnits = units
    .filter(c4LocalUnitFootprintChanged)
    .sort(c4LocalUnitSeedOrder);
  return resizedUnits[0]?.node.id ?? null;
}

function packC4LocalInflateUnits(
  units: readonly C4LocalLayoutUnit[],
  anchorId: string | null = null,
) {
  const localPlacements = placeC4LocalInflateUnits(units, anchorId);
  if (localPlacements) return localPlacements;

  const placements = new Map<string, C4LayoutBox>();
  if (units.length === 0) return placements;

  const rows: Array<{
    units: C4LocalLayoutUnit[];
    centerY: number;
    minY: number;
    maxY: number;
    height: number;
    y: number;
  }> = [];
  for (const unit of [...units].sort(c4LocalUnitSeedOrder)) {
    const unitMinY = unit.seed.y - unit.rowGroupingHeight / 2;
    const unitMaxY = unit.seed.y + unit.rowGroupingHeight / 2;
    const row = rows.find(
      (candidate) =>
        Math.abs(unit.seed.y - candidate.centerY) <= C4_LOCAL_ROW_CLUSTER_GAP,
    );
    if (row) {
      row.units.push(unit);
      row.centerY =
        row.units.reduce((sum, next) => sum + next.seed.y, 0) /
        row.units.length;
      row.minY = Math.min(row.minY, unitMinY);
      row.maxY = Math.max(row.maxY, unitMaxY);
    } else {
      rows.push({
        units: [unit],
        centerY: unit.seed.y,
        minY: unitMinY,
        maxY: unitMaxY,
        height: 0,
        y: 0,
      });
    }
  }

  rows.sort((left, right) => left.centerY - right.centerY);
  for (const row of rows) {
    row.height = Math.max(...row.units.map((unit) => unit.height));
  }
  const anchorRowIndex = rows.findIndex((row) =>
    row.units.some((unit) => unit.node.id === anchorId),
  );
  if (anchorRowIndex >= 0) {
    const anchorUnit = rows[anchorRowIndex]?.units.find(
      (unit) => unit.node.id === anchorId,
    );
    const anchorRow = rows[anchorRowIndex];
    if (anchorUnit && anchorRow) {
      anchorRow.y = anchorUnit.seed.y - anchorRow.height / 2;
      for (let index = anchorRowIndex - 1; index >= 0; index -= 1) {
        const nextRow = rows[index + 1]!;
        const row = rows[index]!;
        row.y = nextRow.y - C4_LOCAL_SIBLING_Y_GAP - row.height;
      }
      for (let index = anchorRowIndex + 1; index < rows.length; index += 1) {
        const previousRow = rows[index - 1]!;
        const row = rows[index]!;
        row.y = previousRow.y + previousRow.height + C4_LOCAL_SIBLING_Y_GAP;
      }
    }
  } else {
    const rowHeights = rows.map((row) => row.height);
    const totalHeight =
      rowHeights.reduce((sum, height) => sum + height, 0) +
      Math.max(0, rows.length - 1) * C4_LOCAL_SIBLING_Y_GAP;
    const levelCenterY =
      units.reduce((sum, unit) => sum + unit.seed.y, 0) / units.length;
    let y = levelCenterY - totalHeight / 2;
    for (const row of rows) {
      row.y = y;
      y += row.height + C4_LOCAL_SIBLING_Y_GAP;
    }
  }

  for (const row of rows) {
    const sortedUnits = [...row.units].sort(
      (left, right) =>
        left.seed.x - right.seed.x || left.node.id.localeCompare(right.node.id),
    );
    const anchorIndex = sortedUnits.findIndex(
      (unit) => unit.node.id === anchorId,
    );
    if (anchorIndex >= 0) {
      const anchorUnit = sortedUnits[anchorIndex]!;
      const anchorX = anchorUnit.seed.x - anchorUnit.width / 2;
      placements.set(anchorUnit.node.id, {
        x: anchorX,
        y: row.y + (row.height - anchorUnit.height) / 2,
        width: anchorUnit.width,
        height: anchorUnit.height,
      });

      let leftCursor = anchorX;
      for (let index = anchorIndex - 1; index >= 0; index -= 1) {
        const unit = sortedUnits[index]!;
        leftCursor -= C4_LOCAL_SIBLING_X_GAP + unit.width;
        placements.set(unit.node.id, {
          x: leftCursor,
          y: row.y + (row.height - unit.height) / 2,
          width: unit.width,
          height: unit.height,
        });
      }

      let rightCursor = anchorX + anchorUnit.width;
      for (
        let index = anchorIndex + 1;
        index < sortedUnits.length;
        index += 1
      ) {
        const unit = sortedUnits[index]!;
        const x = rightCursor + C4_LOCAL_SIBLING_X_GAP;
        placements.set(unit.node.id, {
          x,
          y: row.y + (row.height - unit.height) / 2,
          width: unit.width,
          height: unit.height,
        });
        rightCursor = x + unit.width;
      }
    } else {
      const totalWidth =
        sortedUnits.reduce((sum, unit) => sum + unit.width, 0) +
        Math.max(0, sortedUnits.length - 1) * C4_LOCAL_SIBLING_X_GAP;
      const rowCenterX =
        sortedUnits.reduce((sum, unit) => sum + unit.seed.x, 0) /
        sortedUnits.length;
      let x = rowCenterX - totalWidth / 2;
      for (const unit of sortedUnits) {
        placements.set(unit.node.id, {
          x,
          y: row.y + (row.height - unit.height) / 2,
          width: unit.width,
          height: unit.height,
        });
        x += unit.width + C4_LOCAL_SIBLING_X_GAP;
      }
    }
  }

  return placements;
}

function c4LocalUnitFootprintChanged(unit: C4LocalLayoutUnit): boolean {
  if (!unit.previousBox) return false;
  return (
    Math.abs(unit.width - unit.previousBox.width) > 1 ||
    Math.abs(unit.height - unit.previousBox.height) > 1
  );
}

function placeC4LocalInflateUnits(
  units: readonly C4LocalLayoutUnit[],
  anchorId: string | null,
): Map<string, C4LayoutBox> | null {
  if (!anchorId) return null;
  const anchor = units.find((unit) => unit.node.id === anchorId);
  if (!anchor?.previousBox) return null;

  const placements = new Map<string, C4LayoutBox>();
  const previousAnchor = anchor.previousBox;
  const nextAnchor = c4BoxCenteredAt(anchor.seed, anchor.width, anchor.height);
  const previousAnchorRight = previousAnchor.x + previousAnchor.width;
  const previousAnchorBottom = previousAnchor.y + previousAnchor.height;
  const nextAnchorRight = nextAnchor.x + nextAnchor.width;
  const nextAnchorBottom = nextAnchor.y + nextAnchor.height;
  const boundaryDelta = {
    left: nextAnchor.x - previousAnchor.x,
    right: nextAnchorRight - previousAnchorRight,
    top: nextAnchor.y - previousAnchor.y,
    bottom: nextAnchorBottom - previousAnchorBottom,
  };

  for (const unit of units) {
    let placement = c4BoxCenteredAt(unit.seed, unit.width, unit.height);
    if (unit.node.id === anchor.node.id) {
      placements.set(unit.node.id, nextAnchor);
      continue;
    }

    const delta = c4LocalInflateDeltaForUnit(
      unit,
      anchor.seed,
      previousAnchor,
      boundaryDelta,
    );

    placement = {
      ...placement,
      x: placement.x + delta.x,
      y: placement.y + delta.y,
    };
    placement = c4NudgeBoxOutsideAnchor(
      placement,
      nextAnchor,
      unit.seed,
      delta,
    );
    placements.set(unit.node.id, placement);
  }

  return placements;
}

function c4LocalInflateDeltaForUnit(
  unit: C4LocalLayoutUnit,
  anchorSeed: C4ElkPoint,
  previousAnchor: C4LayoutBox,
  boundaryDelta: { left: number; right: number; top: number; bottom: number },
): C4ElkPoint {
  const previousAnchorRight = previousAnchor.x + previousAnchor.width;
  const previousAnchorBottom = previousAnchor.y + previousAnchor.height;
  const outsideLeft = unit.seed.x < previousAnchor.x;
  const outsideRight = unit.seed.x > previousAnchorRight;
  const outsideTop = unit.seed.y < previousAnchor.y;
  const outsideBottom = unit.seed.y > previousAnchorBottom;
  const horizontalDelta = outsideLeft
    ? boundaryDelta.left
    : outsideRight
      ? boundaryDelta.right
      : 0;
  const verticalDelta = outsideTop
    ? boundaryDelta.top
    : outsideBottom
      ? boundaryDelta.bottom
      : 0;
  if (horizontalDelta !== 0 || verticalDelta !== 0) {
    return { x: horizontalDelta, y: verticalDelta };
  }

  const normalizedDx =
    (unit.seed.x - anchorSeed.x) / Math.max(previousAnchor.width, 1);
  const normalizedDy =
    (unit.seed.y - anchorSeed.y) / Math.max(previousAnchor.height, 1);
  if (Math.abs(normalizedDx) >= Math.abs(normalizedDy)) {
    return {
      x:
        unit.seed.x < anchorSeed.x
          ? boundaryDelta.left
          : unit.seed.x > anchorSeed.x
            ? boundaryDelta.right
            : 0,
      y: 0,
    };
  }
  return {
    x: 0,
    y:
      unit.seed.y < anchorSeed.y
        ? boundaryDelta.top
        : unit.seed.y > anchorSeed.y
          ? boundaryDelta.bottom
          : 0,
  };
}

function c4BoxCenteredAt(
  center: C4ElkPoint,
  width: number,
  height: number,
): C4LayoutBox {
  return {
    x: center.x - width / 2,
    y: center.y - height / 2,
    width,
    height,
  };
}

function c4NudgeBoxOutsideAnchor(
  box: C4LayoutBox,
  anchor: C4LayoutBox,
  seed: C4ElkPoint,
  appliedDelta: C4ElkPoint,
): C4LayoutBox {
  const gap = Math.min(C4_LOCAL_SIBLING_X_GAP, C4_LOCAL_SIBLING_Y_GAP);
  const overlapX =
    Math.min(box.x + box.width, anchor.x + anchor.width) -
    Math.max(box.x, anchor.x);
  const overlapY =
    Math.min(box.y + box.height, anchor.y + anchor.height) -
    Math.max(box.y, anchor.y);
  if (overlapX <= 0 || overlapY <= 0) return box;

  const anchorCenter = {
    x: anchor.x + anchor.width / 2,
    y: anchor.y + anchor.height / 2,
  };
  const preferHorizontal =
    Math.abs(appliedDelta.x) > Math.abs(appliedDelta.y) ||
    (Math.abs(appliedDelta.x) === Math.abs(appliedDelta.y) &&
      Math.abs(seed.x - anchorCenter.x) >= Math.abs(seed.y - anchorCenter.y));
  if (preferHorizontal) {
    return {
      ...box,
      x:
        seed.x < anchorCenter.x
          ? anchor.x - gap - box.width
          : anchor.x + anchor.width + gap,
    };
  }
  return {
    ...box,
    y:
      seed.y < anchorCenter.y
        ? anchor.y - gap - box.height
        : anchor.y + anchor.height + gap,
  };
}

function c4LocalUnitSeedOrder(
  left: C4LocalLayoutUnit,
  right: C4LocalLayoutUnit,
) {
  return (
    left.seed.y - right.seed.y ||
    left.seed.x - right.seed.x ||
    left.node.id.localeCompare(right.node.id)
  );
}

function c4MeasuredNodeDimensions(
  node: DiagramNode,
  nodeDimensions?: ReadonlyMap<string, C4NodeDimensions>,
): C4NodeDimensions {
  const measured = nodeDimensions?.get(node.id);
  return {
    width: c4PositiveDimension(measured?.width, C4_NODE_WIDTH),
    height: c4PositiveDimension(measured?.height, estimateC4NodeHeight(node)),
  };
}

function c4PositiveDimension(value: number | undefined, fallback: number) {
  return typeof value === "number" && Number.isFinite(value) && value > 0
    ? value
    : fallback;
}

function c4LocalFallbackPoint(index: number): C4ElkPoint {
  return {
    x: index * (C4_NODE_WIDTH + C4_LOCAL_SIBLING_X_GAP),
    y: 0,
  };
}

function c4EmptyLocalLayout(): C4LocalLayoutResult {
  return {
    entries: [],
    bbox: { x: 0, y: 0, width: 0, height: 0 },
  };
}

function c4LayoutEntriesBbox(entries: readonly C4LayoutEntry[]): C4LayoutBox {
  if (entries.length === 0) return { x: 0, y: 0, width: 0, height: 0 };
  const minX = Math.min(...entries.map((entry) => entry.x));
  const minY = Math.min(...entries.map((entry) => entry.y));
  const maxX = Math.max(...entries.map((entry) => entry.x + entry.width));
  const maxY = Math.max(...entries.map((entry) => entry.y + entry.height));
  return { x: minX, y: minY, width: maxX - minX, height: maxY - minY };
}

async function routeC4FixedLayoutEdges(
  layoutNodes: C4LayoutEntry[],
  relationships: DiagramRelationship[],
  wasmUrl?: string,
): Promise<C4LayoutResult> {
  const boxesById = new Map(
    layoutNodes.map((entry) => [
      entry.node.id,
      {
        x: entry.x,
        y: entry.y,
        width: entry.width,
        height: entry.height,
      },
    ]),
  );
  const edgeRelationships = relationships.filter(
    (relationship) =>
      boxesById.has(relationship.from) && boxesById.has(relationship.to),
  );

  if (edgeRelationships.length === 0) {
    return {
      nodes: layoutNodes,
      edgeSections: new Map(),
      edgeLabels: new Map(),
    };
  }

  const routing = await routeOrthogonalConnectors({
    connectors: edgeRelationships.map((relationship) => {
      const label = relationship.hideLabel
        ? undefined
        : (relationship.label ?? relationship.semanticKind);
      return {
        id: relationship.id,
        sourceId: relationship.from,
        targetId: relationship.to,
        labelSize: label ? estimateC4EdgeLabelDimensions(label) : undefined,
      };
    }),
    boxesById,
    jobs: c4LibavoidRoutingJobs(layoutNodes, edgeRelationships),
    routingOptions: C4_LIBAVOID_ROUTING_OPTIONS,
    wasmUrl,
    labelObstacles: c4EdgeLabelNodeObstacles(layoutNodes),
    labelOptions: {
      candidateStep: C4_EDGE_LABEL_CANDIDATE_STEP,
      labelGutter: C4_EDGE_LABEL_LABEL_GUTTER,
      nodeGutter: C4_EDGE_LABEL_NODE_GUTTER,
    },
  });

  return {
    nodes: layoutNodes,
    edgeSections: routing.sections,
    edgeLabels: routing.labels,
  };
}

const C4_LIBAVOID_ROUTING_OPTIONS = {
  routingType: "orthogonal",
  segmentPenalty: 10,
  shapeBufferDistance: 14,
  idealNudgingDistance: 8,
  portDirectionPenalty: 100,
  nudgeOrthogonalSegmentsConnectedToShapes: true,
  nudgeSharedPathsWithCommonEndPoint: true,
  performUnifyingNudgingPreprocessingStep: true,
  selfLoopHandling: "fallback",
} as const;
const C4_LIBAVOID_DENSE_EDGE_THRESHOLD = 48;
const C4_LIBAVOID_DENSE_EDGE_BATCH_SIZE = 16;

function c4LibavoidRoutingJobs(
  layoutNodes: readonly C4LayoutEntry[],
  edgeRelationships: readonly DiagramRelationship[],
) {
  return c4LibavoidRoutingScopes(layoutNodes, edgeRelationships).flatMap(
    (scope) => {
      if (scope.edgeRelationships.length === 0) return [];
      if (
        scope.edgeRelationships.length < C4_LIBAVOID_DENSE_EDGE_THRESHOLD
      ) {
        return [
          {
            graph: c4FlatLibavoidGraphFromLayout(
              scope.layoutNodes,
              scope.edgeRelationships,
              scope.axis,
            ),
            connectorIds: scope.edgeRelationships.map((relationship) =>
              relationship.id
            ),
            axis: scope.axis,
          },
        ];
      }
      const jobs = [];
      for (
        let startIndex = 0;
        startIndex < scope.edgeRelationships.length;
        startIndex += C4_LIBAVOID_DENSE_EDGE_BATCH_SIZE
      ) {
        const batch = scope.edgeRelationships.slice(
          startIndex,
          startIndex + C4_LIBAVOID_DENSE_EDGE_BATCH_SIZE,
        );
        jobs.push({
          graph: c4FlatLibavoidGraphFromLayout(
            scope.layoutNodes,
            batch,
            scope.axis,
            scope.edgeRelationships,
          ),
          connectorIds: batch.map((relationship) => relationship.id),
          axis: scope.axis,
        });
      }
      return jobs;
    },
  );
}

function c4LibavoidRoutingScopes(
  layoutNodes: readonly C4LayoutEntry[],
  edgeRelationships: readonly DiagramRelationship[],
) {
  const entriesById = new Map(
    layoutNodes.map((entry) => [entry.node.id, entry]),
  );
  const groupedEdges = new Map<string, DiagramRelationship[]>();
  const globalEdges: DiagramRelationship[] = [];

  const routingNodesForEdges = (
    entries: readonly C4LayoutEntry[],
    edges: readonly DiagramRelationship[],
  ) => {
    const endpointIds = new Set(
      edges.flatMap((relationship) => [relationship.from, relationship.to]),
    );
    return entries.filter(
      (entry) => !entry.expandedGroup || endpointIds.has(entry.node.id),
    );
  };

  for (const edge of edgeRelationships) {
    const scopeId = c4DeepestCommonExpandedAncestorId(
      edge.from,
      edge.to,
      entriesById,
    );
    if (!scopeId) {
      globalEdges.push(edge);
      continue;
    }
    const edges = groupedEdges.get(scopeId) ?? [];
    edges.push(edge);
    groupedEdges.set(scopeId, edges);
  }

  return [
    ...[...groupedEdges.entries()].map(([scopeId, edges]) => ({
      scopeId,
      axis: c4ChildLayoutAxis(entriesById.get(scopeId)?.node),
      edgeRelationships: edges,
      layoutNodes: routingNodesForEdges(
        layoutNodes.filter(
          (entry) =>
            entry.node.id !== scopeId &&
            c4IsLayoutDescendantOf(entry.node.id, scopeId, entriesById),
        ),
        edges,
      ),
    })),
    {
      scopeId: null,
      axis: c4ChildLayoutAxis(),
      edgeRelationships: globalEdges,
      layoutNodes: routingNodesForEdges(layoutNodes, globalEdges),
    },
  ];
}

function c4DeepestCommonExpandedAncestorId(
  fromId: string,
  toId: string,
  entriesById: ReadonlyMap<string, C4LayoutEntry>,
): string | null {
  const toAncestors = c4ExpandedAncestorIds(toId, entriesById);
  for (const ancestorId of c4ExpandedAncestorIds(fromId, entriesById)) {
    if (toAncestors.has(ancestorId)) return ancestorId;
  }
  return null;
}

function c4ExpandedAncestorIds(
  nodeId: string,
  entriesById: ReadonlyMap<string, C4LayoutEntry>,
): Set<string> {
  const ancestors = new Set<string>();
  let current = entriesById.get(nodeId);
  while (current?.node.parentId) {
    const parent = entriesById.get(current.node.parentId);
    if (!parent) break;
    if (parent.expandedGroup) ancestors.add(parent.node.id);
    current = parent;
  }
  return ancestors;
}

function c4IsLayoutDescendantOf(
  nodeId: string,
  ancestorId: string,
  entriesById: ReadonlyMap<string, C4LayoutEntry>,
): boolean {
  let current = entriesById.get(nodeId);
  while (current?.node.parentId) {
    if (current.node.parentId === ancestorId) return true;
    current = entriesById.get(current.node.parentId);
  }
  return false;
}

function c4FlatLibavoidGraphFromLayout(
  layoutNodes: readonly C4LayoutEntry[],
  edgeRelationships: readonly DiagramRelationship[],
  axis: C4LayoutAxis,
  portRelationships = edgeRelationships,
): LibavoidGraph {
  const bbox = c4LayoutEntriesBbox(layoutNodes);
  const entriesById = new Map(
    layoutNodes.map((entry) => [entry.node.id, entry]),
  );
  const portsByNodeId = c4RoutingPortsByNodeId(
    entriesById,
    portRelationships,
    axis,
  );
  return {
    id: "software-map-c4-fixed-flat",
    width: bbox.x + bbox.width + 80,
    height: bbox.y + bbox.height + 80,
    children: layoutNodes.map((entry) => ({
      id: entry.node.id,
      x: entry.x,
      y: entry.y,
      width: entry.width,
      height: entry.height,
      ports: portsByNodeId.get(entry.node.id),
    })),
    edges: edgeRelationships.map((relationship) => {
      const refs = c4RoutingEndpointRefs(relationship, entriesById, axis);
      return {
        id: relationship.id,
        source: relationship.from,
        target: relationship.to,
        sourcePort: refs.sourcePortId,
        targetPort: refs.targetPortId,
      };
    }),
  };
}

async function runC4ElkLayout(
  nodes: DiagramNode[],
  relationships: DiagramRelationship[],
  nodeDimensions?: ReadonlyMap<string, C4NodeDimensions>,
  options: {
    previousLayout?: InlineC4LayoutResult;
    axis?: C4LayoutAxis;
  } = {},
): Promise<C4LayoutEntry[]> {
  const previousGeometry = c4PreviousLayoutGeometry(options.previousLayout);
  const previousCenters = previousGeometry.centers;
  const previousBoxes = previousGeometry.boxes;
  const layoutAxis = options.axis ?? c4ChildLayoutAxis();
  const sorted = [...nodes].sort((left, right) =>
    compareC4NodesForLayout(left, right, previousCenters, layoutAxis),
  );
  if (sorted.length === 0) return [];

  const nodeIds = new Set(sorted.map((node) => node.id));
  const nodesById = new Map(sorted.map((node) => [node.id, node]));
  const childIdsByParentId = new Map<string, string[]>();
  for (const node of sorted) {
    if (!node.parentId || !nodeIds.has(node.parentId)) continue;
    const children = childIdsByParentId.get(node.parentId) ?? [];
    children.push(node.id);
    childIdsByParentId.set(node.parentId, children);
  }
  const rootNodes = sorted.filter((node) => {
    if (!node.parentId) return true;
    const parent = nodesById.get(node.parentId);
    return !parent?.expanded;
  });
  const visibleRelationships = relationships.filter(
    (relationship) =>
      nodeIds.has(relationship.from) && nodeIds.has(relationship.to),
  );
  const layerSpacing = c4EdgeLabelLayerSpacing(visibleRelationships);
  const layoutHintsByNodeId = new Map<string, C4LayoutEntry>(
    sorted.map((node) => {
      const hint = previousBoxes.get(node.id);
      const dimensions = c4MeasuredNodeDimensions(node, nodeDimensions);
      return [
        node.id,
        {
          node,
          x: hint?.x ?? 0,
          y: hint?.y ?? 0,
          width: hint?.width ?? dimensions.width,
          height: hint?.height ?? dimensions.height,
        },
      ];
    }),
  );
  const portsByNodeId = c4SchemaPortsByNodeId(
    layoutHintsByNodeId,
    visibleRelationships,
  );
  // ELK ignores its cycle-breaking strategy for cross-hierarchy cycles under
  // INCLUDE_CHILDREN. Orient each edge along this layer's configured axis so
  // ELK cannot flip the previous arrangement during expansion.
  const elkEdges = visibleRelationships.map((relationship) => {
    const label = relationship.hideLabel
      ? undefined
      : (relationship.label ?? relationship.semanticKind);
    const from = c4PreviousProxyCenter(
      relationship.from,
      nodesById,
      previousCenters,
    );
    const to = c4PreviousProxyCenter(
      relationship.to,
      nodesById,
      previousCenters,
    );
    const reversed = Boolean(
      from &&
      to &&
      c4PointAxisCoordinate(from, layoutAxis) >
        c4PointAxisCoordinate(to, layoutAxis),
    );
    const refs = c4SchemaEndpointRefs(relationship, layoutHintsByNodeId);
    const sourceRef = refs.sourcePortId ?? relationship.from;
    const targetRef = refs.targetPortId ?? relationship.to;
    return {
      id: relationship.id,
      sources: [reversed ? targetRef : sourceRef],
      targets: [reversed ? sourceRef : targetRef],
      labels: label
        ? [
            {
              id: `${relationship.id}:label`,
              text: label,
              ...estimateC4EdgeLabelDimensions(label),
              layoutOptions: {
                "org.eclipse.elk.edgeLabels.placement": "TAIL",
                "org.eclipse.elk.edgeLabels.inline": "true",
              },
            },
          ]
        : undefined,
    };
  });
  const result = (await c4Elk.layout({
    id: "software-map-c4",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": c4ElkDirectionForAxis(layoutAxis),
      "elk.hierarchyHandling": "INCLUDE_CHILDREN",
      "elk.spacing.nodeNode": "72",
      "elk.layered.spacing.nodeNodeBetweenLayers": String(
        layerSpacing[layoutAxis],
      ),
      "elk.edgeRouting": "ORTHOGONAL",
      "elk.padding": "[top=40,left=40,bottom=40,right=40]",
      "org.eclipse.elk.layered.edgeLabels.centerLabelPlacementStrategy":
        "SPACE_EFFICIENT_LAYER",
      "elk.layered.nodePlacement.strategy": "BRANDES_KOEPF",
      "elk.layered.considerModelOrder.strategy": "NODES_AND_EDGES",
      // With a previous/desired layout, model order and interactive positions
      // encode the on-screen arrangement so expansion can preserve the mental
      // map while ELK still owns layered orthogonal routing.
      ...(previousCenters.size > 0
        ? {
            "org.eclipse.elk.interactiveLayout": "true",
            "org.eclipse.elk.layered.cycleBreaking.strategy": "INTERACTIVE",
            "org.eclipse.elk.layered.layering.strategy": "INTERACTIVE",
            "org.eclipse.elk.layered.crossingMinimization.semiInteractive":
              "true",
            "org.eclipse.elk.layered.crossingMinimization.forceNodeModelOrder":
              "true",
            "org.eclipse.elk.separateConnectedComponents": "false",
          }
        : {}),
    },
    children: rootNodes.map((node) =>
      c4ElkNodeForSnapshot(node, {
        childIdsByParentId,
        layoutHints: previousBoxes,
        nodeDimensions,
        nodesById,
        portsByNodeId,
        layerSpacing,
      }),
    ),
    edges: elkEdges,
  } as never)) as unknown as C4ElkLayoutGraph;

  return collectC4ElkLayoutEntries({
    children: result.children ?? [],
    nodesById,
    offset: { x: 0, y: 0 },
  });
}

interface C4ElkLayoutGraph {
  id: string;
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  children?: C4ElkLayoutNode[];
  layoutOptions?: Record<string, string>;
}

type C4ElkDirection = "RIGHT" | "DOWN";
type C4LayoutAxis = "horizontal" | "vertical";

function c4EdgeLabelLayerSpacing(
  relationships: readonly DiagramRelationship[],
): Record<C4LayoutAxis, number> {
  const spacing: Record<C4LayoutAxis, number> = {
    horizontal: C4_EDGE_LABEL_MAX_WIDTH + C4_EDGE_LABEL_NODE_GUTTER * 2,
    vertical: 64,
  };
  for (const relationship of relationships) {
    const label = relationship.hideLabel
      ? undefined
      : (relationship.label ?? relationship.semanticKind);
    if (!label) continue;
    const dimensions = estimateC4EdgeLabelDimensions(label);
    spacing.horizontal = Math.max(
      spacing.horizontal,
      Math.ceil(dimensions.width + C4_EDGE_LABEL_NODE_GUTTER * 2),
    );
    spacing.vertical = Math.max(
      spacing.vertical,
      Math.ceil(dimensions.height + C4_EDGE_LABEL_NODE_GUTTER * 2),
    );
  }
  return spacing;
}

// Keep the layer policy here. Layout, position preservation, and edge routing
// all translate this axis for their own APIs.
function c4ChildLayoutAxis(
  parent?: Pick<DiagramNode, "type">,
): C4LayoutAxis {
  return parent?.type === "softwareSystem" ? "vertical" : "horizontal";
}

function c4ElkDirectionForAxis(axis: C4LayoutAxis): C4ElkDirection {
  return axis === "vertical" ? "DOWN" : "RIGHT";
}

function c4PointAxisCoordinate(point: C4ElkPoint, axis: C4LayoutAxis) {
  return axis === "vertical" ? point.y : point.x;
}

interface C4ElkLayoutNode extends C4ElkLayoutGraph {
  id: string;
  ports?: C4ElkPort[];
}

interface C4ElkPort {
  id: string;
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  properties?: Record<string, unknown>;
}

function c4ElkNodeForSnapshot(
  node: DiagramNode,
  context: {
    childIdsByParentId: ReadonlyMap<string, readonly string[]>;
    layoutHints?: ReadonlyMap<string, C4LayoutBox>;
    nodeDimensions?: ReadonlyMap<string, C4NodeDimensions>;
    nodesById: ReadonlyMap<string, DiagramNode>;
    portsByNodeId?: ReadonlyMap<string, C4ElkPort[]>;
    layerSpacing: Readonly<Record<C4LayoutAxis, number>>;
  },
  parentOffset: C4ElkPoint = { x: 0, y: 0 },
): C4ElkLayoutNode {
  const hint = context.layoutHints?.get(node.id);
  const nodeOffset = hint ? { x: hint.x, y: hint.y } : parentOffset;
  const dimensions = c4MeasuredNodeDimensions(node, context.nodeDimensions);
  const children = node.expanded
    ? (context.childIdsByParentId.get(node.id) ?? [])
        .map((childId) => context.nodesById.get(childId))
        .filter((child): child is DiagramNode => Boolean(child))
        .map((child) => c4ElkNodeForSnapshot(child, context, nodeOffset))
    : [];
  return {
    id: node.id,
    ...(hint
      ? {
          x: hint.x - parentOffset.x,
          y: hint.y - parentOffset.y,
        }
      : {}),
    width: hint?.width ?? dimensions.width,
    height: hint?.height ?? dimensions.height,
    ports: context.portsByNodeId?.get(node.id),
    children: children.length > 0 ? children : undefined,
    layoutOptions:
      children.length > 0
        ? {
            "elk.direction": c4ElkDirectionForAxis(c4ChildLayoutAxis(node)),
            "elk.layered.spacing.nodeNodeBetweenLayers": String(
              context.layerSpacing[c4ChildLayoutAxis(node)],
            ),
            "elk.padding": "[top=70,left=36,bottom=36,right=36]",
          }
        : undefined,
  };
}

function collectC4ElkLayoutEntries({
  children,
  nodesById,
  offset,
}: {
  children: readonly C4ElkLayoutNode[];
  nodesById: ReadonlyMap<string, DiagramNode>;
  offset: C4ElkPoint;
}): C4LayoutEntry[] {
  return children.flatMap((child) => {
    const node = nodesById.get(child.id);
    if (!node) return [];
    const x = offset.x + (child.x ?? 0);
    const y = offset.y + (child.y ?? 0);
    const childOffset = { x, y };
    return [
      {
        node,
        x,
        y,
        width: child.width ?? C4_NODE_WIDTH,
        height: child.height ?? estimateC4NodeHeight(node),
        expandedGroup: node.expanded && (child.children?.length ?? 0) > 0,
      },
      ...collectC4ElkLayoutEntries({
        children: child.children ?? [],
        nodesById,
        offset: childOffset,
      }),
    ];
  });
}

function c4EdgeLabelNodeObstacles(
  layoutNodes: readonly C4LayoutEntry[],
): RoutingObstacle[] {
  return layoutNodes.map((entry) => ({
    x: entry.x,
    y: entry.y,
    width: entry.width,
    height: entry.expandedGroup
      ? Math.min(entry.height, C4_EXPANDED_GROUP_LABEL_HEADER_HEIGHT)
      : entry.height,
  }));
}

function c4LayoutSignature(
  nodes: readonly DiagramNode[],
  relationships: readonly DiagramRelationship[],
  nodeDimensions?: ReadonlyMap<string, C4NodeDimensions> | null,
) {
  const nodeSignatures = nodes
    .map((node) =>
      [
        node.id,
        node.type,
        node.dataStoreKind ?? "",
        node.label,
        node.parentId ?? "",
        node.expanded ? "expanded" : "",
        node.description ?? "",
        node.changeStatus ?? "",
        node.boundary ? "boundary" : "",
        node.childCount ?? "",
        c4DataStoreSchemaSignature(node),
        nodeDimensions?.get(node.id)?.width ?? "",
        nodeDimensions?.get(node.id)?.height ?? "",
      ].join("\u001f"),
    )
    .sort();
  const relationshipSignatures = relationships
    .map((relationship) =>
      [
        relationship.id,
        relationship.from,
        relationship.to,
        relationship.label ?? "",
        relationship.kind,
        relationship.semanticKind ?? "",
        relationship.hideLabel ? "hide-label" : "",
      ].join("\u001f"),
    )
    .sort();
  return [...nodeSignatures, "\u001d", ...relationshipSignatures].join(
    "\u001e",
  );
}

function c4PreviousInlineLayoutForRelationships(input: {
  previousLayout: InlineC4LayoutResult | null | undefined;
  previousRelationships:
    | readonly DiagramRelationship[]
    | null
    | undefined;
  currentRelationships: readonly DiagramRelationship[];
}): InlineC4LayoutResult | undefined {
  if (!input.previousLayout || !input.previousRelationships) return undefined;
  return c4RelationshipTopologySignature(input.previousRelationships) ===
    c4RelationshipTopologySignature(input.currentRelationships)
    ? input.previousLayout
    : undefined;
}

function c4RelationshipTopologySignature(
  relationships: readonly DiagramRelationship[],
) {
  return relationships
    .map((relationship) =>
      [
        relationship.id,
        relationship.from,
        relationship.to,
        relationship.fromSchemaEndpointKind ?? "",
        ...(relationship.fromSchemaFieldPath ?? []),
        relationship.toSchemaEndpointKind ?? "",
        ...(relationship.toSchemaFieldPath ?? []),
      ].join("\u001f"),
    )
    .sort()
    .join("\u001e");
}

function c4MeasurementKey(nodes: DiagramNode[]) {
  return nodes
    .map((node) =>
      [
        node.id,
        node.label,
        node.type,
        node.dataStoreKind ?? "",
        node.changeStatus ?? "",
        node.description ?? "",
        node.file ?? "",
        node.line ?? "",
        node.boundary ? "boundary" : "",
        node.childCount ?? "",
        c4DataStoreSchemaSignature(node),
      ].join("\u001f"),
    )
    .join("\u001e");
}

function c4DataStoreSchemaSignature(node: DiagramNode): string {
  return (node.dataStoreSchemaSections ?? [])
    .map((section) =>
      [
        section.id,
        section.label,
        section.kind,
        section.key ?? "",
        ...section.rows.map((row) =>
          [
            row.id,
            row.label,
            row.depth ?? "",
            row.type ?? "",
            row.example ?? "",
            row.primaryKey ? "pk" : "",
            row.foreignKey ? "fk" : "",
          ].join("\u001d"),
        ),
      ].join("\u001c"),
    )
    .join("\u001b");
}

function c4DimensionsEqual(
  left: ReadonlyMap<string, C4NodeDimensions> | null,
  right: ReadonlyMap<string, C4NodeDimensions>,
) {
  if (!left || left.size !== right.size) return false;
  for (const [id, rightDimensions] of right) {
    const leftDimensions = left.get(id);
    if (
      !leftDimensions ||
      leftDimensions.width !== rightDimensions.width ||
      leftDimensions.height !== rightDimensions.height
    ) {
      return false;
    }
  }
  return true;
}

function C4NodeMeasurementLayer({
  nodes,
  measurementKey,
  onMeasure,
}: {
  nodes: DiagramNode[];
  measurementKey: string;
  onMeasure: (dimensions: ReadonlyMap<string, C4NodeDimensions>) => void;
}) {
  const refs = useRef(new Map<string, HTMLDivElement>());
  const nodesRef = useRef(nodes);
  nodesRef.current = nodes;

  useLayoutEffect(() => {
    const measuredNodes = nodesRef.current;
    if (measuredNodes.length === 0) {
      onMeasure(new Map());
      return;
    }
    const measure = () => {
      const dimensions = new Map<string, C4NodeDimensions>();
      for (const node of measuredNodes) {
        const element = refs.current.get(node.id);
        if (!element) return;
        const rect = element.getBoundingClientRect();
        dimensions.set(node.id, {
          width: Math.ceil(rect.width),
          height: Math.ceil(rect.height),
        });
      }
      onMeasure(dimensions);
    };
    return scheduleC4NodeMeasurements(measure);
  }, [measurementKey, onMeasure]);

  return (
    <div className="software-map-c4-measure-layer" aria-hidden="true">
      {nodes.map((node) => (
        <div
          key={node.id}
          ref={(element) => {
            if (element) {
              refs.current.set(node.id, element);
            } else {
              refs.current.delete(node.id);
            }
          }}
          className={[
            "software-map-c4-measure-node",
            `software-map-c4-measure-node--${node.type}`,
          ].join(" ")}
        >
          <SoftwareMapNodeFrame node={node} selected={false} />
        </div>
      ))}
    </div>
  );
}

interface C4NodeMeasurementScheduler {
  requestFrame(callback: FrameRequestCallback): number;
  cancelFrame(frame: number): void;
  setTimer(callback: () => void, delay: number): number;
  clearTimer(timer: number): void;
}

function scheduleC4NodeMeasurements(
  measure: () => void,
  scheduler: C4NodeMeasurementScheduler = {
    requestFrame: (callback) => requestAnimationFrame(callback),
    cancelFrame: (frame) => cancelAnimationFrame(frame),
    setTimer: (callback, delay) => window.setTimeout(callback, delay),
    clearTimer: (timer) => window.clearTimeout(timer),
  },
): () => void {
  let disposed = false;
  const run = () => {
    if (!disposed) measure();
  };

  // Native editor tabs can mount while Chromium reports the workbench page as
  // hidden. Animation frames are paused in that state, so take the initial
  // layout measurement synchronously and use frames only for refinement.
  run();
  const frame = scheduler.requestFrame(run);
  const followUpMeasurements = [120, 500].map((delay) =>
    scheduler.setTimer(run, delay),
  );

  return () => {
    disposed = true;
    scheduler.cancelFrame(frame);
    for (const timeout of followUpMeasurements) scheduler.clearTimer(timeout);
  };
}

function c4EdgeHandles(
  source: C4LayoutBox,
  target: C4LayoutBox,
  section: RoutingSection,
) {
  const spatialSides = c4ConnectionSides(source, target);
  const sourceSide = c4RoutingSideForBorderPoint(
    source,
    section.startPoint,
    spatialSides.source,
  );
  const targetSide = c4RoutingSideForBorderPoint(
    target,
    section.endPoint,
    spatialSides.target,
  );
  return {
    sourceHandle: `source-${sourceSide}`,
    targetHandle: `target-${targetSide}`,
  };
}

function c4ConnectionSides(
  source: C4LayoutBox,
  target: C4LayoutBox,
): { source: RoutingSide; target: RoutingSide } {
  const sourceCenter = {
    x: source.x + source.width / 2,
    y: source.y + source.height / 2,
  };
  const targetCenter = {
    x: target.x + target.width / 2,
    y: target.y + target.height / 2,
  };
  const dx = targetCenter.x - sourceCenter.x;
  const dy = targetCenter.y - sourceCenter.y;
  const normalizedDx =
    Math.abs(dx) / Math.max((source.width + target.width) / 2, 1);
  const normalizedDy =
    Math.abs(dy) / Math.max((source.height + target.height) / 2, 1);
  if (normalizedDx >= normalizedDy) {
    return dx >= 0
      ? { source: "right", target: "left" }
      : { source: "left", target: "right" };
  }
  return dy >= 0
    ? { source: "bottom", target: "top" }
    : { source: "top", target: "bottom" };
}

function c4RoutingSideForBorderPoint(
  box: C4LayoutBox,
  point: C4ElkPoint,
  fallback: RoutingSide,
): RoutingSide {
  const distances: Array<[RoutingSide, number]> = [
    ["left", Math.abs(point.x - box.x)],
    ["right", Math.abs(point.x - (box.x + box.width))],
    ["top", Math.abs(point.y - box.y)],
    ["bottom", Math.abs(point.y - (box.y + box.height))],
  ];
  distances.sort((left, right) => left[1] - right[1]);
  const closest = distances[0];
  return closest && closest[1] <= 1 ? closest[0] : fallback;
}

function estimateC4NodeHeight(node: DiagramNode): number {
  const dataStoreShape =
    node.type === "dataStore"
      ? softwareMapDataStoreShape(node.dataStoreKind)
      : undefined;
  const storageShapeExtraHeight =
    dataStoreShape === "cylinder" || dataStoreShape === "bucket"
      ? 70
      : dataStoreShape === "folder"
        ? 40
        : 0;
  const minHeight = dataStoreShape ? 168 : C4_MIN_NODE_HEIGHT;
  const titleLines = Math.max(
    1,
    Math.ceil(node.label.length / C4_TITLE_CHARS_PER_LINE),
  );
  const descriptionLines = node.description
    ? Math.max(
        1,
        Math.ceil(node.description.length / C4_DESCRIPTION_CHARS_PER_LINE),
      )
    : 0;
  const metaCount =
    (node.file ? 1 : 0) +
    (node.childCount && node.childCount > 0 ? 1 : 0) +
    (node.boundary ? 1 : 0);
  const metaRows = metaCount > 0 ? Math.ceil(metaCount / 2) : 0;
  const verticalGaps =
    2 + (descriptionLines > 0 ? 1 : 0) + (metaRows > 0 ? 1 : 0);
  const schemaRows = (node.dataStoreSchemaSections ?? []).reduce(
    (total, section) => total + section.rows.length + 1 + (section.key ? 1 : 0),
    0,
  );
  const schemaHeight =
    schemaRows > 0
      ? 28 + schemaRows * 32 + (node.dataStoreSchemaSections?.length ?? 0) * 10
      : 0;

  return Math.max(
    schemaHeight > 0 ? Math.max(minHeight, 320) : minHeight,
    24 +
      storageShapeExtraHeight +
      14 +
      titleLines * 19 +
      descriptionLines * 17 +
      metaRows * 20 +
      schemaHeight +
      verticalGaps * 7,
  );
}

function estimateC4EdgeLabelDimensions(label: string): C4LabelDimensions {
  const words = label.trim().split(/\s+/).filter(Boolean);
  let lineCount = 1;
  let currentLineLength = 0;
  let longestLineLength = 0;

  for (const word of words) {
    const nextLength =
      currentLineLength === 0
        ? word.length
        : currentLineLength + 1 + word.length;
    if (currentLineLength > 0 && nextLength > C4_EDGE_LABEL_CHARS_PER_LINE) {
      longestLineLength = Math.max(longestLineLength, currentLineLength);
      lineCount += 1;
      currentLineLength = word.length;
    } else {
      currentLineLength = nextLength;
    }

    while (currentLineLength > C4_EDGE_LABEL_CHARS_PER_LINE) {
      longestLineLength = Math.max(
        longestLineLength,
        C4_EDGE_LABEL_CHARS_PER_LINE,
      );
      lineCount += 1;
      currentLineLength -= C4_EDGE_LABEL_CHARS_PER_LINE;
    }
  }

  longestLineLength = Math.max(longestLineLength, currentLineLength, 1);
  return {
    width: Math.min(
      C4_EDGE_LABEL_MAX_WIDTH,
      longestLineLength * 6.4 + C4_EDGE_LABEL_HORIZONTAL_PADDING,
    ),
    height:
      lineCount * C4_EDGE_LABEL_LINE_HEIGHT + C4_EDGE_LABEL_VERTICAL_PADDING,
  };
}

function c4EdgeColor(
  kind: CanvasRelationshipKind,
  semanticKind?: string,
): string {
  if (semanticKind === "primary") return "var(--accent)";
  if (
    kind === "call" ||
    semanticKind === "http" ||
    semanticKind === "published"
  ) {
    return "var(--rpc)";
  }
  return "var(--map-edge)";
}

function c4EdgeDasharray(
  kind: CanvasRelationshipKind,
  sourceNodeType?: CanvasNodeKind,
  targetNodeType?: CanvasNodeKind,
  semanticKind?: string,
): string | undefined {
  if (
    semanticKind === "async" ||
    semanticKind === "return" ||
    semanticKind === "optional" ||
    semanticKind === "forbidden"
  ) {
    return "5 4";
  }
  if (semanticKind) return undefined;
  if (kind === "semantic") {
    return sourceNodeType === "codeElement" || targetNodeType === "codeElement"
      ? "1 5"
      : undefined;
  }
  return undefined;
}

type C4SchemaSide = "left" | "right";

function c4SchemaPortsByNodeId(
  entriesById: ReadonlyMap<string, C4LayoutEntry>,
  edgeRelationships: readonly DiagramRelationship[],
): Map<string, C4ElkPort[]> {
  const portsByNodeId = new Map<string, C4ElkPort[]>();

  for (const relationship of edgeRelationships) {
    const refs = c4SchemaEndpointRefs(relationship, entriesById);
    const sourceEntry = entriesById.get(relationship.from);
    if (
      sourceEntry &&
      refs.sourcePortId &&
      relationship.fromSchemaEndpointKind
    ) {
      c4AddSchemaPort({
        entry: sourceEntry,
        fieldPath: relationship.fromSchemaFieldPath ?? [],
        kind: relationship.fromSchemaEndpointKind,
        laneKey: `from:${relationship.id}`,
        portId: refs.sourcePortId,
        portsByNodeId,
        side: refs.sourceSide,
      });
    }

    const targetEntry = entriesById.get(relationship.to);
    if (targetEntry && refs.targetPortId && relationship.toSchemaEndpointKind) {
      c4AddSchemaPort({
        entry: targetEntry,
        fieldPath: relationship.toSchemaFieldPath ?? [],
        kind: relationship.toSchemaEndpointKind,
        laneKey: `to:${relationship.id}`,
        portId: refs.targetPortId,
        portsByNodeId,
        side: refs.targetSide,
      });
    }
  }

  return portsByNodeId;
}

function c4RoutingPortsByNodeId(
  entriesById: ReadonlyMap<string, C4LayoutEntry>,
  edgeRelationships: readonly DiagramRelationship[],
  axis: C4LayoutAxis,
): Map<string, C4ElkPort[]> {
  const portsByNodeId = c4SchemaPortsByNodeId(entriesById, edgeRelationships);

  for (const relationship of edgeRelationships) {
    const refs = c4RoutingEndpointRefs(relationship, entriesById, axis);
    const sourceEntry = entriesById.get(relationship.from);
    if (
      sourceEntry &&
      refs.sourcePortId &&
      !relationship.fromSchemaEndpointKind
    ) {
      c4AddRoutingPort({
        entry: sourceEntry,
        portId: refs.sourcePortId,
        portsByNodeId,
        side: refs.sourceSide,
      });
    }
    const targetEntry = entriesById.get(relationship.to);
    if (
      targetEntry &&
      refs.targetPortId &&
      !relationship.toSchemaEndpointKind
    ) {
      c4AddRoutingPort({
        entry: targetEntry,
        portId: refs.targetPortId,
        portsByNodeId,
        side: refs.targetSide,
      });
    }
  }

  c4SpreadRoutingPorts(entriesById, edgeRelationships, portsByNodeId, axis);
  return portsByNodeId;
}

function c4SpreadRoutingPorts(
  entriesById: ReadonlyMap<string, C4LayoutEntry>,
  edgeRelationships: readonly DiagramRelationship[],
  portsByNodeId: ReadonlyMap<string, C4ElkPort[]>,
  axis: C4LayoutAxis,
) {
  const laneByPortId = new Map<string, number>();
  for (const relationship of edgeRelationships) {
    const source = entriesById.get(relationship.from);
    const target = entriesById.get(relationship.to);
    if (!source || !target) continue;
    const refs = c4RoutingEndpointRefs(relationship, entriesById, axis);
    if (refs.sourcePortId && !relationship.fromSchemaEndpointKind) {
      laneByPortId.set(
        refs.sourcePortId,
        c4RoutingLaneCoordinate(refs.sourceSide, target),
      );
    }
    if (refs.targetPortId && !relationship.toSchemaEndpointKind) {
      laneByPortId.set(
        refs.targetPortId,
        c4RoutingLaneCoordinate(refs.targetSide, source),
      );
    }
  }

  for (const [nodeId, ports] of portsByNodeId) {
    const entry = entriesById.get(nodeId);
    if (!entry) continue;
    const positions = distributePorts(
      entry,
      ports.flatMap((port) => {
        const lane = laneByPortId.get(port.id);
        const side = c4RoutingSideFromElk(port.properties?.["port.side"]);
        return lane === undefined || !side ? [] : [{ id: port.id, lane, side }];
      }),
    );
    for (const port of ports) {
      const position = positions.get(port.id);
      if (position) Object.assign(port, position);
    }
  }
}

function c4RoutingSideFromElk(side: unknown): RoutingSide | null {
  return side === "NORTH"
    ? "top"
    : side === "SOUTH"
      ? "bottom"
      : side === "EAST"
        ? "right"
        : side === "WEST"
          ? "left"
          : null;
}

function c4RoutingLaneCoordinate(side: RoutingSide, peer: C4LayoutEntry) {
  return side === "top" || side === "bottom"
    ? peer.x + peer.width / 2
    : peer.y + peer.height / 2;
}

function c4AddRoutingPort({
  entry,
  portId,
  portsByNodeId,
  side,
}: {
  entry: C4LayoutEntry;
  portId: string;
  portsByNodeId: Map<string, C4ElkPort[]>;
  side: RoutingSide;
}) {
  const horizontal = side === "left" || side === "right";
  const ports = portsByNodeId.get(entry.node.id) ?? [];
  ports.push({
    id: portId,
    x: side === "right" ? entry.width : horizontal ? 0 : entry.width / 2,
    y: side === "bottom" ? entry.height : horizontal ? entry.height / 2 : 0,
    width: 0,
    height: 0,
    properties: {
      "port.side":
        side === "right"
          ? "EAST"
          : side === "left"
            ? "WEST"
            : side === "bottom"
              ? "SOUTH"
              : "NORTH",
    },
  });
  portsByNodeId.set(entry.node.id, ports);
}

function c4RoutingEndpointRefs(
  relationship: DiagramRelationship,
  entriesById: ReadonlyMap<string, C4LayoutEntry>,
  axis: C4LayoutAxis,
): {
  sourcePortId?: string;
  sourceSide: RoutingSide;
  targetPortId?: string;
  targetSide: RoutingSide;
} {
  const source = entriesById.get(relationship.from);
  const target = entriesById.get(relationship.to);
  const connectionSides =
    source && target
      ? c4AxisConnectionSides(source, target, axis)
      : c4DefaultConnectionSides(axis);
  const schemaRefs = c4SchemaEndpointRefs(relationship, entriesById);
  const sourceSide = relationship.fromSchemaEndpointKind
    ? schemaRefs.sourceSide
    : connectionSides.source;
  const targetSide = relationship.toSchemaEndpointKind
    ? schemaRefs.targetSide
    : connectionSides.target;
  return {
    sourcePortId: relationship.fromSchemaEndpointKind
      ? schemaRefs.sourcePortId
      : source
        ? c4RoutingPortId(
            relationship.from,
            relationship.id,
            "source",
            sourceSide,
          )
        : undefined,
    sourceSide,
    targetPortId: relationship.toSchemaEndpointKind
      ? schemaRefs.targetPortId
      : target
        ? c4RoutingPortId(
            relationship.to,
            relationship.id,
            "target",
            targetSide,
          )
        : undefined,
    targetSide,
  };
}

function c4AxisConnectionSides(
  source: C4LayoutBox,
  target: C4LayoutBox,
  axis: C4LayoutAxis,
): { source: RoutingSide; target: RoutingSide } {
  const sourceCenter = {
    x: source.x + source.width / 2,
    y: source.y + source.height / 2,
  };
  const targetCenter = {
    x: target.x + target.width / 2,
    y: target.y + target.height / 2,
  };
  if (axis === "vertical") {
    return targetCenter.y >= sourceCenter.y
      ? { source: "bottom", target: "top" }
      : { source: "top", target: "bottom" };
  }
  return targetCenter.x >= sourceCenter.x
    ? { source: "right", target: "left" }
    : { source: "left", target: "right" };
}

function c4DefaultConnectionSides(axis: C4LayoutAxis): {
  source: RoutingSide;
  target: RoutingSide;
} {
  return axis === "vertical"
    ? { source: "bottom", target: "top" }
    : { source: "right", target: "left" };
}

function c4RoutingPortId(
  nodeId: string,
  edgeId: string,
  role: "source" | "target",
  side: RoutingSide,
) {
  return `${nodeId}::edge-port:${role}:${side}:${edgeId}`;
}

function c4AddSchemaPort({
  entry,
  fieldPath,
  kind,
  laneKey,
  portId,
  portsByNodeId,
  side,
}: {
  entry: C4LayoutEntry;
  fieldPath: readonly string[];
  kind: "field" | "header";
  laneKey: string;
  portId: string;
  portsByNodeId: Map<string, C4ElkPort[]>;
  side: C4SchemaSide;
}) {
  const y =
    kind === "header"
      ? c4SchemaHeaderCenterY(entry.node, entry.height, laneKey)
      : c4SchemaFieldCenterY(entry.node, entry.height, fieldPath);
  if (typeof y !== "number") return;
  const ports = portsByNodeId.get(entry.node.id) ?? [];
  ports.push({
    id: portId,
    x: side === "right" ? entry.width : 0,
    y,
    width: 0,
    height: 0,
    properties: {
      "port.side": side === "right" ? "EAST" : "WEST",
    },
  });
  portsByNodeId.set(entry.node.id, ports);
}

function c4SchemaEndpointRefs(
  relationship: DiagramRelationship,
  entriesById: ReadonlyMap<string, C4LayoutEntry>,
): {
  sourcePortId?: string;
  sourceSide: C4SchemaSide;
  targetPortId?: string;
  targetSide: C4SchemaSide;
} {
  const source = entriesById.get(relationship.from);
  const target = entriesById.get(relationship.to);
  const sourceSide =
    source && target ? c4SchemaPortSide(source, target, "source") : "right";
  const targetSide =
    source && target ? c4SchemaPortSide(target, source, "target") : "left";
  // Emit a port id only when c4AddSchemaPort can place that port, so an edge
  // never references a port that port registration skipped (ELK rejects the
  // whole graph on a dangling port reference).
  return {
    sourcePortId:
      relationship.fromSchemaEndpointKind &&
      c4SchemaPortPlaceable(
        source,
        relationship.fromSchemaEndpointKind,
        relationship.fromSchemaFieldPath ?? [],
      )
        ? c4SchemaPortId({
            edgeId: relationship.id,
            fieldPath: relationship.fromSchemaFieldPath ?? [],
            kind: relationship.fromSchemaEndpointKind,
            nodeId: relationship.from,
            side: sourceSide,
          })
        : undefined,
    sourceSide,
    targetPortId:
      relationship.toSchemaEndpointKind &&
      c4SchemaPortPlaceable(
        target,
        relationship.toSchemaEndpointKind,
        relationship.toSchemaFieldPath ?? [],
      )
        ? c4SchemaPortId({
            edgeId: relationship.id,
            fieldPath: relationship.toSchemaFieldPath ?? [],
            kind: relationship.toSchemaEndpointKind,
            nodeId: relationship.to,
            side: targetSide,
          })
        : undefined,
    targetSide,
  };
}

function c4SchemaPortPlaceable(
  entry: C4LayoutEntry | undefined,
  kind: "field" | "header",
  fieldPath: readonly string[],
): boolean {
  if (!entry) return false;
  if (kind === "header") return true;
  return (
    typeof c4SchemaFieldCenterY(entry.node, entry.height, fieldPath) ===
    "number"
  );
}

function c4SchemaPortId({
  edgeId,
  fieldPath,
  kind,
  nodeId,
  side,
}: {
  edgeId: string;
  fieldPath: readonly string[];
  kind: "field" | "header";
  nodeId: string;
  side: C4SchemaSide;
}) {
  const fieldKey = fieldPath.length > 0 ? fieldPath.join(".") : "header";
  return `${nodeId}::schema-port:${kind}:${fieldKey}:${side}:${edgeId}`;
}

function c4SchemaPortSide(
  source: C4LayoutEntry,
  target: C4LayoutEntry,
  role: "source" | "target",
): C4SchemaSide {
  const sourceCenter = {
    x: source.x + source.width / 2,
    y: source.y + source.height / 2,
  };
  const targetCenter = {
    x: target.x + target.width / 2,
    y: target.y + target.height / 2,
  };
  if (sourceCenter.x === targetCenter.x && sourceCenter.y === targetCenter.y) {
    return role === "source" ? "right" : "left";
  }
  return target.x + target.width / 2 >= source.x + source.width / 2
    ? "right"
    : "left";
}

function c4SchemaHeaderCenterY(
  node: DiagramNode,
  height: number,
  laneKey: string,
): number {
  return (
    c4SchemaBlockTop(node, height) + 15 + c4SchemaHeaderLaneOffset(laneKey)
  );
}

function c4SchemaHeaderLaneOffset(laneKey: string): number {
  const lanes = [-12, -9, -6, -3, 0, 3, 6, 9, 12];
  let hash = 2166136261;
  for (let index = 0; index < laneKey.length; index += 1) {
    hash ^= laneKey.charCodeAt(index);
    hash = Math.imul(hash, 16777619) >>> 0;
  }
  return lanes[hash % lanes.length] ?? 0;
}

function c4SchemaFieldCenterY(
  node: DiagramNode,
  height: number,
  fieldPath: readonly string[],
): number | undefined {
  const sections = node.dataStoreSchemaSections ?? [];
  let y = c4SchemaBlockTop(node, height);
  for (const section of sections) {
    y += 30;
    if (section.key) y += 30;
    const rowIndex = section.rows.findIndex(
      (row) => row.id.split(":").slice(1).join(".") === fieldPath.join("."),
    );
    if (rowIndex >= 0) return y + rowIndex * 30 + 15;
    y += section.rows.length * 30 + 8;
  }
  return undefined;
}

function c4SchemaBlockTop(
  node: DiagramNode,
  height: number,
): number {
  const sections = node.dataStoreSchemaSections ?? [];
  const blockHeight =
    sections.reduce(
      (total, section) =>
        total + 30 + (section.key ? 30 : 0) + section.rows.length * 30,
      0,
    ) +
    Math.max(0, sections.length - 1) * 8;
  return Math.max(0, height - blockHeight - 18);
}

function SoftwareMapC4Edge(props: ReactFlowEdgeProps) {
  const hoveredNodeId = useContext(C4HoveredNodeContext);
  const data = props.data as C4MapEdgeData;
  const label = data.relationship.hideLabel
    ? undefined
    : typeof props.label === "string"
      ? props.label
      : (data.relationship.label ?? data.relationship.semanticKind);
  const points = edgePointsFromSection(data.section);
  if (points.length < 2) return null;
  const forbidden = data.relationship.semanticKind === "forbidden";
  const displayPoints = forbidden ? c4PolylineWithoutEnd(points, 12) : points;
  const path = c4PolylinePath(displayPoints);
  const hitPath = forbidden ? c4PolylinePath(points) : path;
  const stopPoint = forbidden ? displayPoints.at(-1) : undefined;
  const sourcePoint = points[0]!;
  const labelPoint = data.labelPoint;
  const openRelationship = (
    event: ReactMouseEvent<Element> | ReactKeyboardEvent<Element>,
  ) => {
    if (!data.onOpenRelationship) return;
    if (hasTextSelectionWithin(event.currentTarget)) {
      event.stopPropagation();
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    data.onOpenRelationship(data.relationship.id);
  };
  return (
    <>
      {data.operationState && data.operationState !== "inactive" ? (
        <path
          d={path}
          className={[
            "software-map-c4-edge-highlight",
            `software-map-c4-edge-highlight--${data.operationState}`,
          ].join(" ")}
        />
      ) : null}
      <BaseEdge
        path={path}
        markerStart={props.markerStart}
        markerEnd={props.markerEnd}
        style={props.style}
        interactionWidth={props.interactionWidth}
      />
      {stopPoint ? (
        <g
          className="software-map-c4-edge-stop"
          transform={`translate(${stopPoint.x} ${stopPoint.y})`}
          aria-hidden="true"
        >
          <circle r="7" />
          <path d="M -4 -4 L 4 4 M 4 -4 L -4 4" />
        </g>
      ) : null}
      <path
        d={hitPath}
        className="software-map-c4-edge-hit-area"
        onClick={openRelationship}
      />
      <EdgeLabelRenderer>
        <span
          aria-hidden="true"
          className={[
            "software-map-c4-edge-endpoint",
            hoveredNodeId === data.relationship.from
              ? "software-map-c4-edge-endpoint--hovered"
              : "",
          ]
            .filter(Boolean)
            .join(" ")}
          data-endpoint="source"
          style={{
            transform: `translate(-50%, -50%) translate(${sourcePoint.x}px, ${sourcePoint.y}px)`,
          }}
        />
        <div
          className="software-map-c4-edge-comment-target nodrag nopan"
          style={{
            transform: `translate(-50%, -50%) translate(${labelPoint.x}px, ${labelPoint.y}px)`,
          }}
        >
          {label ? (
            data.onOpenRelationship ? (
              <span
                role="button"
                tabIndex={0}
                className={[
                  "software-map-c4-edge-label",
                  "software-map-c4-edge-label--button",
                  data.selectedNodeAttached
                    ? "software-map-c4-edge-label--selected-node"
                    : "",
                  data.operationState
                    ? `software-map-c4-edge-label--${data.operationState}`
                    : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
                onClick={openRelationship}
                onKeyDown={(event) => {
                  if (event.key !== "Enter" && event.key !== " ") return;
                  openRelationship(event);
                }}
              >
                {label}
              </span>
            ) : (
              <span
                className={[
                  "software-map-c4-edge-label",
                  data.selectedNodeAttached
                    ? "software-map-c4-edge-label--selected-node"
                    : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
              >
                {label}
              </span>
            )
          ) : null}
        </div>
      </EdgeLabelRenderer>
    </>
  );
}

function c4EdgeLabelPoint(
  labelPosition: RoutingLabel | undefined,
  labelDimensions: C4LabelDimensions | undefined,
  fallbackPoints: C4ElkPoint[],
): C4ElkPoint {
  const fallback = polylineMidpoint(fallbackPoints);
  if (
    labelPosition &&
    Number.isFinite(labelPosition.x) &&
    Number.isFinite(labelPosition.y)
  ) {
    const width = Number.isFinite(labelPosition.width)
      ? labelPosition.width
      : (labelDimensions?.width ?? 0);
    const height = Number.isFinite(labelPosition.height)
      ? labelPosition.height
      : (labelDimensions?.height ?? 0);
    return {
      x: labelPosition.x + width / 2,
      y: labelPosition.y + height / 2,
    };
  }
  return fallback;
}

function c4PolylinePath(points: C4ElkPoint[]): string {
  const [first, ...rest] = points;
  if (!first) return "";
  return [
    `M ${first.x} ${first.y}`,
    ...rest.map((point) => `L ${point.x} ${point.y}`),
  ].join(" ");
}

function c4PolylineWithoutEnd(points: C4ElkPoint[], trim: number): C4ElkPoint[] {
  const result = points.map((point) => ({ ...point }));
  let remaining = trim;
  while (result.length > 1 && remaining > 0) {
    const end = result.at(-1)!;
    const start = result.at(-2)!;
    const length = Math.hypot(end.x - start.x, end.y - start.y);
    if (length > remaining) {
      const ratio = (length - remaining) / length;
      result[result.length - 1] = {
        x: start.x + (end.x - start.x) * ratio,
        y: start.y + (end.y - start.y) * ratio,
      };
      return result;
    }
    remaining -= length;
    result.pop();
  }
  return result;
}

const C4_HANDLE_POSITIONS = [
  ["left", Position.Left],
  ["top", Position.Top],
  ["right", Position.Right],
  ["bottom", Position.Bottom],
] as const;

function C4NodeHandles() {
  return C4_HANDLE_POSITIONS.flatMap(([side, position]) =>
    (["target", "source"] as const).map((type) => (
      <Handle
        key={`${type}-${side}`}
        id={`${type}-${side}`}
        type={type}
        position={position}
        className="software-map-c4-handle"
      />
    )),
  );
}

function SoftwareMapC4GroupNode({
  data,
}: ReactFlowNodeProps<C4MapFlowGroupNode>) {
  return (
    <div
      className={[
        "software-map-c4-group-shell",
        `software-map-c4-group-shell--${data.node.type}`,
        data.selected ? "selected" : "",
        data.node.changeStatus && data.node.changeStatus !== "unchanged"
          ? `software-map-c4-group-shell--${data.node.changeStatus}`
          : "",
      ]
        .filter(Boolean)
        .join(" ")}
      onClick={(event) => {
        if (hasTextSelectionWithin(event.currentTarget)) {
          event.stopPropagation();
          return;
        }
        event.preventDefault();
        event.stopPropagation();
        data.onSelect?.(data.node);
      }}
      onDoubleClickCapture={(event) => {
        if (hasTextSelectionWithin(event.currentTarget)) {
          event.stopPropagation();
        }
      }}
    >
      <C4NodeHandles />
      <div className="software-map-c4-group-title software-map-c4-group-title--world">
        <span>{softwareMapNodeTypeLabel(data.node)}</span>
        <strong>{data.node.label}</strong>
        <SoftwareMapChangeBadge
          status={data.node.changeStatus}
          additions={data.node.additions}
          deletions={data.node.deletions}
        />
      </div>
    </div>
  );
}

function SoftwareMapC4Node({ data }: ReactFlowNodeProps<C4MapFlowNode>) {
  return (
    <div
      className={["software-map-c4-node-shell", "nodrag", "nopan"]
        .filter(Boolean)
        .join(" ")}
      onDoubleClickCapture={(event) => {
        if (hasTextSelectionWithin(event.currentTarget)) {
          event.stopPropagation();
          return;
        }
        event.preventDefault();
        event.stopPropagation();
        if (data.node.expanded) {
          data.onCollapseNode?.(data.node);
        } else {
          data.onExpandNode?.(data.node);
        }
      }}
    >
      <C4NodeHandles />
      <SoftwareMapNodeFrame
        node={data.node}
        selected={data.selected}
        onSelect={data.onSelect}
        onExpandNode={data.onExpandNode}
      />
    </div>
  );
}

function SoftwareMapNodeFrame({
  node,
  selected,
  as: Element = "div",
  className,
  children,
  onSelect,
  onExpandNode,
}: {
  node: DiagramNode;
  selected: boolean;
  as?: "button" | "div";
  className?: string;
  children?: ReactNode;
  onSelect?: (node: DiagramNode) => void;
  onExpandNode?: (node: DiagramNode) => void;
}) {
  const isCodeElement = node.type === "codeElement";
  const dataStoreShape =
    node.type === "dataStore"
      ? softwareMapDataStoreShape(node.dataStoreKind)
      : undefined;
  const hasExpandedDataStoreSchema =
    (node.type === "dataStore" || node.type === "dataStoreCollection") &&
    Boolean(node.dataStoreSchemaSections?.length);
  const props = {
    className: [
      "software-map-node",
      "nodrag",
      "nopan",
      `software-map-node--${node.type}`,
      node.type === "dataStore" && node.dataStoreKind
        ? `software-map-node--dataStoreKind-${node.dataStoreKind}`
        : "",
      dataStoreShape
        ? `software-map-node--dataStoreShape-${dataStoreShape}`
        : "",
      node.changeStatus && node.changeStatus !== "unchanged"
        ? `software-map-node--${node.changeStatus}`
        : "",
      selected ? "selected" : "",
      node.boundary ? "boundary" : "",
      hasExpandedDataStoreSchema
        ? "software-map-node--has-data-store-schema"
        : "",
      className ?? "",
    ]
      .filter(Boolean)
      .join(" "),
    onClick: (event: ReactMouseEvent<HTMLElement>) => {
      if (hasTextSelectionWithin(event.currentTarget)) {
        event.stopPropagation();
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      onSelect?.(node);
    },
    onDoubleClick: (event: ReactMouseEvent<HTMLElement>) => {
      if (hasTextSelectionWithin(event.currentTarget)) {
        event.stopPropagation();
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      onExpandNode?.(node);
    },
  };
  return (
    <Element
      {...props}
      {...(Element === "button"
        ? {
            type: "button",
            "aria-label": `${softwareMapNodeTypeLabel(node)}: ${node.label}`,
          }
        : {
            role: "group",
            "aria-label": `${softwareMapNodeTypeLabel(node)}: ${node.label}`,
          })}
    >
      {isCodeElement ? (
        <div className="software-map-code-element-head">
          <code className="software-map-node-label--world">{node.label}</code>
          <SoftwareMapChangeBadge
            status={node.changeStatus}
            additions={node.additions}
            deletions={node.deletions}
          />
        </div>
      ) : (
        <>
          <div className="software-map-node-kicker">
            <div className="software-map-node-type">
              {softwareMapNodeTypeLabel(node)}
            </div>
            <SoftwareMapChangeBadge
              status={node.changeStatus}
              additions={node.additions}
              deletions={node.deletions}
            />
          </div>
          <h4 className="software-map-node-label--world">{node.label}</h4>
        </>
      )}
      {!isCodeElement && node.description && (
        <p className="software-map-node-description--world">
          {node.description}
        </p>
      )}
      {!isCodeElement && (
        <div className="software-map-node-meta">
          {node.file && (
            <span>
              {node.file}
              {typeof node.line === "number" ? `:L${node.line}` : ""}
            </span>
          )}
          {typeof node.childCount === "number" && node.childCount > 0 && (
            <span>{node.childCount} children</span>
          )}
          {node.boundary && <span>boundary</span>}
        </div>
      )}
      {children}
      {hasExpandedDataStoreSchema && (
        <SoftwareMapDataStoreSchema
          sections={node.dataStoreSchemaSections ?? []}
        />
      )}
    </Element>
  );
}

function SoftwareMapDataStoreSchema({
  sections,
}: {
  sections: DiagramDataStoreSchemaSection[];
}) {
  return (
    <div className="software-map-data-store-schema">
      {sections.map((section) => (
        <section
          key={section.id}
          className={`software-map-data-store-schema-section software-map-data-store-schema-section--${section.kind}`}
        >
          <header className="software-map-data-store-schema-section-header">
            <span>{section.kind}</span>
            <strong>{section.label}</strong>
          </header>
          {section.key && (
            <div className="software-map-data-store-schema-key">
              {section.key}
            </div>
          )}
          <div className="software-map-data-store-schema-rows">
            {section.rows.map((row) => (
              <div
                key={row.id}
                className={[
                  "software-map-data-store-schema-row",
                  row.primaryKey
                    ? "software-map-data-store-schema-row--primary"
                    : "",
                  row.state && row.state !== "inactive"
                    ? `software-map-data-store-schema-row--${row.state}`
                    : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
                style={
                  {
                    "--software-map-schema-row-depth": row.depth ?? 0,
                  } as CSSProperties
                }
              >
                <span className="software-map-data-store-schema-row-name">
                  {row.primaryKey && <strong>PK</strong>}
                  {row.foreignKey && (
                    <strong className="foreign-key">FK</strong>
                  )}
                  {row.label}
                </span>
                <span className="software-map-data-store-schema-row-type">
                  {row.type ?? row.example ?? "object"}
                </span>
              </div>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}

function SoftwareMapChangeBadge({
  status,
  additions,
  deletions,
}: {
  status?: ChangeState;
  additions?: number;
  deletions?: number;
}) {
  const visibleAdditions = visibleSoftwareMapChangeCount(additions);
  const visibleDeletions = visibleSoftwareMapChangeCount(deletions);
  const hasCounts = Boolean(visibleAdditions || visibleDeletions);
  const hasChangeStatus = Boolean(status && status !== "unchanged");
  if (!hasCounts && !hasChangeStatus) return null;
  if (!hasCounts) {
    return (
      <span
        className="software-map-change-badge software-map-change-badge--empty"
        aria-hidden="true"
      />
    );
  }
  return (
    <span className="software-map-change-badge">
      {visibleAdditions ? (
        <span className="software-map-change-count software-map-change-count--added">
          +{visibleAdditions}
        </span>
      ) : null}
      {visibleDeletions ? (
        <span className="software-map-change-count software-map-change-count--removed">
          -{visibleDeletions}
        </span>
      ) : null}
    </span>
  );
}

function visibleSoftwareMapChangeCount(count?: number) {
  return typeof count === "number" && Number.isFinite(count) && count > 0
    ? count
    : 0;
}
