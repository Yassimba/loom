import assert from "node:assert/strict";
import test from "node:test";

import {
  edgePointsFromSection,
  positionRouteLabels,
} from "../skills/code-diagram/scripts/src/canvas/routing/labels.ts";
import {
  fallbackOrthogonalRoute,
  normalizeOrthogonalRoute,
  pointOnBoxBorder,
} from "../skills/code-diagram/scripts/src/canvas/routing/orthogonal.ts";
import { distributePorts } from "../skills/code-diagram/scripts/src/canvas/routing/ports.ts";

test("shared routing preserves C4 geometry", () => {
  const source = { x: 0, y: 0, width: 40, height: 40 };
  const target = { x: 120, y: 80, width: 40, height: 40 };
  const valid = {
    sourcePoint: { x: 40, y: 20 },
    bendPoints: [
      { x: 80, y: 20.004 },
      { x: 80, y: 100 },
    ],
    targetPoint: { x: 120, y: 100 },
  };
  assert.deepEqual(normalizeOrthogonalRoute(valid, source, target), {
    sourcePoint: { x: 40, y: 20 },
    bendPoints: [
      { x: 80, y: 20 },
      { x: 80, y: 100 },
    ],
    targetPoint: { x: 120, y: 100 },
  });
  assert.equal(
    normalizeOrthogonalRoute(
      {
        sourcePoint: { x: 40, y: 20 },
        bendPoints: [{ x: 70, y: 50 }],
        targetPoint: { x: 120, y: 100 },
      },
      source,
      target,
    ),
    null,
  );

  const fallback = fallbackOrthogonalRoute(source, target, "horizontal");
  assert.deepEqual(
    edgePointsFromSection({
      startPoint: fallback.sourcePoint,
      bendPoints: fallback.bendPoints,
      endPoint: fallback.targetPoint,
    }),
    [
      { x: 40, y: 20 },
      { x: 80, y: 20 },
      { x: 80, y: 100 },
      { x: 120, y: 100 },
    ],
  );
  assert.equal(pointOnBoxBorder(fallback.sourcePoint, source), true);
  assert.equal(pointOnBoxBorder(fallback.targetPoint, target), true);
  assert.deepEqual(
    distributePorts({ width: 100, height: 60 }, [
      { id: "far", side: "right", lane: 90 },
      { id: "near", side: "right", lane: 10 },
      { id: "top", side: "top", lane: 20 },
    ]),
    new Map([
      ["near", { x: 100, y: 20 }],
      ["far", { x: 100, y: 40 }],
      ["top", { x: 50, y: 0 }],
    ]),
  );

  const sections = new Map([
    [
      "edge-a",
      {
        startPoint: { x: 40, y: 20 },
        bendPoints: [{ x: 80, y: 20 }],
        endPoint: { x: 120, y: 20 },
      },
    ],
  ]);
  const labels = new Map([["edge-a", { x: 70, y: 10, width: 20, height: 10 }]]);
  assert.deepEqual(
    positionRouteLabels(sections, labels, [], {
      candidateStep: 28,
      labelGutter: 8,
      nodeGutter: 14,
    }).get("edge-a"),
    { x: 70, y: 15, width: 20, height: 10 },
  );
  assert.deepEqual(
    positionRouteLabels(sections, labels, [], {
      candidateStep: 28,
      labelGutter: 8,
      nodeGutter: 14,
      routeGap: 8,
    }).get("edge-a"),
    { x: 70, y: 2, width: 20, height: 10 },
  );
});
