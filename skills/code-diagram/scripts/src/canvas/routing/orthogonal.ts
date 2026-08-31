import type {
  OrthogonalRoute,
  RoutingAxis,
  RoutingBox,
  RoutingPoint,
} from "./model";

const DEFAULT_TOLERANCE = 0.01;

export function normalizeOrthogonalRoute<Route extends OrthogonalRoute>(
  route: Route,
  source: RoutingBox,
  target: RoutingBox,
  tolerance = DEFAULT_TOLERANCE,
): Route | null {
  const points = [route.sourcePoint, ...route.bendPoints, route.targetPoint];

  const normalized = [{ ...points[0]! }];
  for (const point of points.slice(1)) {
    const previous = normalized.at(-1)!;
    const next = { ...point };
    if (Math.abs(next.x - previous.x) <= tolerance) {
      next.x = previous.x;
    } else if (Math.abs(next.y - previous.y) <= tolerance) {
      next.y = previous.y;
    } else {
      return null;
    }
    normalized.push(next);
  }

  if (
    !pointOnBoxBorder(normalized[0]!, source, tolerance) ||
    !pointOnBoxBorder(normalized.at(-1)!, target, tolerance)
  ) {
    return null;
  }

  return {
    ...route,
    sourcePoint: normalized[0]!,
    bendPoints: normalized.slice(1, -1),
    targetPoint: normalized.at(-1)!,
  };
}

export function fallbackOrthogonalRoute(
  source: RoutingBox,
  target: RoutingBox,
  axis: RoutingAxis,
): OrthogonalRoute {
  const sourceCenter = boxCenter(source);
  const targetCenter = boxCenter(target);

  if (axis === "vertical") {
    const downward = targetCenter.y >= sourceCenter.y;
    const sourcePoint = {
      x: sourceCenter.x,
      y: downward ? source.y + source.height : source.y,
    };
    const targetPoint = {
      x: targetCenter.x,
      y: downward ? target.y : target.y + target.height,
    };
    const middleY = (sourcePoint.y + targetPoint.y) / 2;
    return {
      sourcePoint,
      bendPoints:
        sourcePoint.x === targetPoint.x
          ? []
          : [
              { x: sourcePoint.x, y: middleY },
              { x: targetPoint.x, y: middleY },
            ],
      targetPoint,
    };
  }

  const rightward = targetCenter.x >= sourceCenter.x;
  const sourcePoint = {
    x: rightward ? source.x + source.width : source.x,
    y: sourceCenter.y,
  };
  const targetPoint = {
    x: rightward ? target.x : target.x + target.width,
    y: targetCenter.y,
  };
  const middleX = (sourcePoint.x + targetPoint.x) / 2;
  return {
    sourcePoint,
    bendPoints:
      sourcePoint.y === targetPoint.y
        ? []
        : [
            { x: middleX, y: sourcePoint.y },
            { x: middleX, y: targetPoint.y },
          ],
    targetPoint,
  };
}

export function pointOnBoxBorder(
  point: RoutingPoint,
  box: RoutingBox,
  tolerance = DEFAULT_TOLERANCE,
) {
  const withinX =
    point.x >= box.x - tolerance &&
    point.x <= box.x + box.width + tolerance;
  const withinY =
    point.y >= box.y - tolerance &&
    point.y <= box.y + box.height + tolerance;
  return (
    (withinY &&
      (Math.abs(point.x - box.x) <= tolerance ||
        Math.abs(point.x - (box.x + box.width)) <= tolerance)) ||
    (withinX &&
      (Math.abs(point.y - box.y) <= tolerance ||
        Math.abs(point.y - (box.y + box.height)) <= tolerance))
  );
}

function boxCenter(box: RoutingBox): RoutingPoint {
  return {
    x: box.x + box.width / 2,
    y: box.y + box.height / 2,
  };
}
