import type {
  RoutingLabel,
  RoutingObstacle,
  RoutingPoint,
  RoutingSection,
} from "./model";

export interface LabelPlacementOptions {
  candidateStep: number;
  labelGutter: number;
  nodeGutter: number;
  routeGap?: number;
}

export function positionRouteLabels(
  edgeSections: ReadonlyMap<string, RoutingSection>,
  edgeLabels: ReadonlyMap<string, RoutingLabel>,
  nodeObstacles: readonly RoutingObstacle[],
  options: LabelPlacementOptions,
): Map<string, RoutingLabel> {
  const positioned = new Map<string, RoutingLabel>();
  const placed: RoutingLabel[] = [];
  for (const edgeId of [...edgeLabels.keys()].sort()) {
    const label = edgeLabels.get(edgeId)!;
    const section = edgeSections.get(edgeId);
    if (!section) {
      positioned.set(edgeId, label);
      placed.push(label);
      continue;
    }
    const points = edgePointsFromSection(section);
    const center = {
      x: label.x + label.width / 2,
      y: label.y + label.height / 2,
    };
    const totalLength = polylineTotalLength(points);
    const projection = projectPointOntoPolyline(center, points);
    const baseDistance = projection?.distance ?? totalLength / 2;
    const candidateDistances = labelCandidateDistances(
      baseDistance,
      totalLength,
      Math.max(options.candidateStep, label.height),
    );
    const candidates = candidateDistances.flatMap((distance) =>
      edgeLabelCandidatesAtDistance(
        points,
        distance,
        label,
        options.routeGap ?? 0,
      ),
    );
    const candidate = candidates.find(
      (next) =>
        !labelOverlapsAny(next, placed, options.labelGutter) &&
        !labelOverlapsAny(next, nodeObstacles, options.nodeGutter),
    ) ??
      lowestCollisionLabelCandidate(
        candidates,
        placed,
        nodeObstacles,
        options,
      );
    positioned.set(edgeId, candidate);
    placed.push(candidate);
  }
  return positioned;
}

export function edgePointsFromSection(
  section: RoutingSection,
): RoutingPoint[] {
  return [section.startPoint, ...(section.bendPoints ?? []), section.endPoint];
}

export function polylineMidpoint(points: readonly RoutingPoint[]): RoutingPoint {
  return polylinePointAtDistance(points, polylineTotalLength(points) / 2);
}

function projectPointOntoPolyline(
  point: RoutingPoint,
  points: readonly RoutingPoint[],
): { distance: number } | null {
  if (points.length < 2) return null;
  let cursor = 0;
  let best: {
    distance: number;
    pointDistance: number;
  } | null = null;
  for (let index = 1; index < points.length; index++) {
    const start = points[index - 1]!;
    const end = points[index]!;
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    const length = Math.hypot(dx, dy);
    if (length === 0) continue;
    const progress = Math.max(
      0,
      Math.min(
        1,
        ((point.x - start.x) * dx + (point.y - start.y) * dy) /
          length * length,
      ),
    );
    const projected = {
      x: start.x + dx * progress,
      y: start.y + dy * progress,
    };
    const pointDistance = Math.hypot(
      point.x - projected.x,
      point.y - projected.y,
    );
    if (!best || pointDistance < best.pointDistance) {
      best = {
        distance: cursor + length * progress,
        pointDistance,
      };
    }
    cursor += length;
  }
  return best;
}

function polylineTotalLength(points: readonly RoutingPoint[]): number {
  let total = 0;
  for (let index = 1; index < points.length; index += 1) {
    const start = points[index - 1];
    const end = points[index];
    if (!start || !end) continue;
    total += Math.hypot(end.x - start.x, end.y - start.y);
  }
  return total;
}

function polylinePointAtDistance(
  points: readonly RoutingPoint[],
  distance: number,
): RoutingPoint {
  if (points.length === 0) return { x: 0, y: 0 };
  if (points.length === 1) return points[0]!;
  let cursor = 0;
  const target = Math.max(0, Math.min(distance, polylineTotalLength(points)));
  for (let index = 1; index < points.length; index += 1) {
    const start = points[index - 1]!;
    const end = points[index]!;
    const length = Math.hypot(end.x - start.x, end.y - start.y);
    if (length === 0) continue;
    if (cursor + length >= target) {
      const progress = (target - cursor) / length;
      return {
        x: start.x + (end.x - start.x) * progress,
        y: start.y + (end.y - start.y) * progress,
      };
    }
    cursor += length;
  }
  return points.at(-1)!;
}

function edgeLabelCandidatesAtDistance(
  points: readonly RoutingPoint[],
  distance: number,
  label: RoutingLabel,
  routeGap: number,
): RoutingLabel[] {
  const sample = polylineSampleAtDistance(points, distance);
  const centered = {
    ...label,
    x: sample.point.x - label.width / 2,
    y: sample.point.y - label.height / 2,
  };
  if (routeGap <= 0) return [centered];
  return sample.horizontal
    ? [
        { ...label, x: centered.x, y: sample.point.y - routeGap - label.height },
        { ...label, x: centered.x, y: sample.point.y + routeGap },
      ]
    : [
        { ...label, x: sample.point.x - routeGap - label.width, y: centered.y },
        { ...label, x: sample.point.x + routeGap, y: centered.y },
      ];
}

function polylineSampleAtDistance(
  points: readonly RoutingPoint[],
  distance: number,
): { point: RoutingPoint; horizontal: boolean } {
  const point = polylinePointAtDistance(points, distance);
  let cursor = 0;
  const target = Math.max(0, Math.min(distance, polylineTotalLength(points)));
  for (let index = 1; index < points.length; index += 1) {
    const start = points[index - 1]!;
    const end = points[index]!;
    const length = Math.hypot(end.x - start.x, end.y - start.y);
    if (length === 0) continue;
    if (cursor + length >= target) {
      return { point, horizontal: start.y === end.y };
    }
    cursor += length;
  }
  const start = points.at(-2);
  const end = points.at(-1);
  return { point, horizontal: Boolean(start && end && start.y === end.y) };
}

function labelCandidateDistances(
  baseDistance: number,
  totalLength: number,
  step: number,
): number[] {
  const distances = [baseDistance];
  const maxSteps = Math.max(1, Math.ceil(totalLength / Math.max(1, step)));
  for (let index = 1; index <= maxSteps; index += 1) {
    distances.push(baseDistance + step * index, baseDistance - step * index);
  }
  return [
    ...new Set(
      distances.map((distance) =>
        Math.max(0, Math.min(totalLength, distance)),
      ),
    ),
  ];
}

function labelOverlapsAny(
  label: RoutingLabel,
  obstacles: readonly RoutingObstacle[],
  gutter: number,
): boolean {
  return obstacles.some((obstacle) => labelBoxesOverlap(label, obstacle, gutter));
}

function lowestCollisionLabelCandidate(
  candidates: RoutingLabel[],
  placedLabels: readonly RoutingLabel[],
  nodeObstacles: readonly RoutingObstacle[],
  options: LabelPlacementOptions,
): RoutingLabel {
  let best = { candidate: candidates[0]!, score: Number.POSITIVE_INFINITY };
  for (const candidate of candidates) {
    const score =
      labelCollisionScore(candidate, placedLabels, options.labelGutter) +
      labelCollisionScore(candidate, nodeObstacles, options.nodeGutter) * 4;
    if (score < best.score) best = { candidate, score };
  }
  return best.candidate;
}

function labelCollisionScore(
  label: RoutingLabel,
  obstacles: readonly RoutingObstacle[],
  gutter: number,
): number {
  return obstacles.reduce(
    (score, obstacle) => score + labelOverlapArea(label, obstacle, gutter),
    0,
  );
}

function labelOverlapArea(
  left: RoutingLabel,
  right: RoutingObstacle,
  gutter: number,
): number {
  const expandedRight = expandBox(right, gutter);
  const overlapWidth = Math.max(
    0,
    Math.min(left.x + left.width, expandedRight.x + expandedRight.width) -
      Math.max(left.x, expandedRight.x),
  );
  const overlapHeight = Math.max(
    0,
    Math.min(left.y + left.height, expandedRight.y + expandedRight.height) -
      Math.max(left.y, expandedRight.y),
  );
  return overlapWidth * overlapHeight;
}

function expandBox(box: RoutingObstacle, gutter: number): RoutingObstacle {
  return {
    x: box.x - gutter,
    y: box.y - gutter,
    width: box.width + gutter * 2,
    height: box.height + gutter * 2,
  };
}

function labelBoxesOverlap(
  left: RoutingLabel,
  right: RoutingObstacle,
  gutter = 0,
): boolean {
  const expandedRight = expandBox(right, gutter);
  return !(
    left.x + left.width <= expandedRight.x ||
    expandedRight.x + expandedRight.width <= left.x ||
    left.y + left.height <= expandedRight.y ||
    expandedRight.y + expandedRight.height <= left.y
  );
}
