import type { RoutingPoint } from "./model";

export type RoutingSide = "left" | "right" | "top" | "bottom";

export interface PortPlacement {
  id: string;
  side: RoutingSide;
  lane: number;
}

export function distributePorts(
  box: { width: number; height: number },
  placements: readonly PortPlacement[],
): Map<string, RoutingPoint> {
  const positions = new Map<string, RoutingPoint>();
  const bySide = new Map<RoutingSide, PortPlacement[]>();
  for (const placement of placements) {
    const sidePorts = bySide.get(placement.side) ?? [];
    sidePorts.push(placement);
    bySide.set(placement.side, sidePorts);
  }

  for (const [side, sidePorts] of bySide) {
    sidePorts.sort(
      (left, right) => left.lane - right.lane || left.id.localeCompare(right.id),
    );
    sidePorts.forEach((port, index) => {
      const position = (index + 1) / (sidePorts.length + 1);
      positions.set(port.id, {
        x:
          side === "right"
            ? box.width
            : side === "left"
              ? 0
              : box.width * position,
        y:
          side === "bottom"
            ? box.height
            : side === "top"
              ? 0
              : box.height * position,
      });
    });
  }

  return positions;
}
