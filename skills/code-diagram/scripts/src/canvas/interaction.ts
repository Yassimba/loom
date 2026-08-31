export interface CanvasInteractionState {
  selectedNodeId: string | null;
  expandedNodeIds: Set<string>;
  viewportFocusNodeId: string | null;
}

export type CanvasInteractionEvent =
  | { type: "select"; nodeId: string }
  | { type: "expand"; nodeId: string; focus?: boolean }
  | {
      type: "collapse";
      nodeId: string;
      expandedNodeIds?: ReadonlySet<string>;
    }
  | { type: "focus"; nodeId: string }
  | { type: "focus-complete"; nodeId: string };

export function createCanvasInteractionState(
  expandedNodeIds: ReadonlySet<string> = new Set(),
): CanvasInteractionState {
  return {
    selectedNodeId: null,
    expandedNodeIds: new Set(expandedNodeIds),
    viewportFocusNodeId: null,
  };
}

export function canvasInteractionReducer(
  state: CanvasInteractionState,
  event: CanvasInteractionEvent,
): CanvasInteractionState {
  if (event.type === "select")
    return { ...state, selectedNodeId: event.nodeId, viewportFocusNodeId: null };
  if (event.type === "expand")
    return {
      ...state,
      selectedNodeId: event.nodeId,
      expandedNodeIds: new Set(state.expandedNodeIds).add(event.nodeId),
      viewportFocusNodeId: event.focus ? event.nodeId : null,
    };
  if (event.type === "collapse") {
    const expandedNodeIds = event.expandedNodeIds
      ? new Set(event.expandedNodeIds)
      : new Set(state.expandedNodeIds);
    expandedNodeIds.delete(event.nodeId);
    return {
      ...state,
      selectedNodeId: event.nodeId,
      expandedNodeIds,
      viewportFocusNodeId: null,
    };
  }
  if (event.type === "focus")
    return { ...state, viewportFocusNodeId: event.nodeId };
  return state.viewportFocusNodeId === event.nodeId
    ? { ...state, viewportFocusNodeId: null }
    : state;
}
