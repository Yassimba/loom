import {
  init as initLibavoid,
  routeEdges,
  type ElkGraph,
} from "@mr_mint/elkjs-libavoid";
import {
  polylineMidpoint,
  positionRouteLabels,
  type LabelPlacementOptions,
} from "./labels";
import type {
  OrthogonalRoute,
  RoutingAxis,
  RoutingBox,
  RoutingLabel,
  RoutingObstacle,
  RoutingSection,
} from "./model";
import {
  fallbackOrthogonalRoute,
  normalizeOrthogonalRoute,
} from "./orthogonal";

export interface OrthogonalConnector {
  id: string;
  sourceId: string;
  targetId: string;
  labelSize?: { width: number; height: number };
}

export interface OrthogonalRoutingJob {
  graph: ElkGraph;
  connectorIds: readonly string[];
  axis: RoutingAxis;
}

export interface OrthogonalRoutingRequest {
  connectors: readonly OrthogonalConnector[];
  boxesById: ReadonlyMap<string, RoutingBox>;
  jobs: readonly OrthogonalRoutingJob[];
  routingOptions: Parameters<typeof routeEdges>[1];
  wasmUrl?: string;
  labelObstacles?: readonly RoutingObstacle[];
  labelOptions?: LabelPlacementOptions;
}

export interface OrthogonalRoutingResult {
  sections: Map<string, RoutingSection>;
  labels: Map<string, RoutingLabel>;
}

export async function routeOrthogonalConnectors(
  request: OrthogonalRoutingRequest,
): Promise<OrthogonalRoutingResult> {
  const rawRoutes = new Map<string, OrthogonalRoute>();
  const axisByConnectorId = new Map<string, RoutingAxis>();
  await initLibavoid(request.wasmUrl);
  for (const job of request.jobs) {
    for (const connectorId of job.connectorIds)
      axisByConnectorId.set(connectorId, job.axis);
    const routed = await routeEdges(job.graph, request.routingOptions);
    for (const [connectorId, route] of routed)
      rawRoutes.set(connectorId, route);
  }

  const sections = new Map<string, RoutingSection>();
  const labels = new Map<string, RoutingLabel>();
  for (const connector of request.connectors) {
    const source = request.boxesById.get(connector.sourceId);
    const target = request.boxesById.get(connector.targetId);
    const axis = axisByConnectorId.get(connector.id);
    if (!source || !target || !axis) continue;
    const raw = rawRoutes.get(connector.id);
    const route =
      (raw ? normalizeOrthogonalRoute(raw, source, target) : null) ??
      fallbackOrthogonalRoute(source, target, axis);
    sections.set(connector.id, {
      startPoint: route.sourcePoint,
      bendPoints: route.bendPoints.length ? route.bendPoints : undefined,
      endPoint: route.targetPoint,
    });
    if (connector.labelSize) {
      const midpoint = polylineMidpoint([
        route.sourcePoint,
        ...route.bendPoints,
        route.targetPoint,
      ]);
      labels.set(connector.id, {
        x: midpoint.x - connector.labelSize.width / 2,
        y: midpoint.y - connector.labelSize.height / 2,
        ...connector.labelSize,
      });
    }
  }

  const positionedLabels = request.labelOptions
    ? positionRouteLabels(
        sections,
        labels,
        request.labelObstacles ?? [],
        request.labelOptions,
      )
    : labels;
  return { sections, labels: positionedLabels };
}
