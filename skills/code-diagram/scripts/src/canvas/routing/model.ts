export interface RoutingPoint {
  x: number;
  y: number;
}

export interface RoutingBox extends RoutingPoint {
  width: number;
  height: number;
}

export interface RoutingSection {
  startPoint: RoutingPoint;
  bendPoints?: RoutingPoint[];
  endPoint: RoutingPoint;
}

export interface OrthogonalRoute {
  sourcePoint: RoutingPoint;
  targetPoint: RoutingPoint;
  bendPoints: RoutingPoint[];
}

export type RoutingLabel = RoutingBox;

export type RoutingObstacle = RoutingBox;

export type RoutingAxis = "horizontal" | "vertical";
