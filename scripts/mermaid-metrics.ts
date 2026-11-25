/**
 * Render Mermaid fixtures with the loom-mermaid renderer found under a given
 * repo root and print cell-level metrics as JSON.
 *
 *   node --experimental-strip-types scripts/mermaid-metrics.ts <root> <fixture.mmd>...
 *
 * Reads only the finished `Canvas` (chars, roles, direction bits, box cells),
 * so the same script measures any renderer revision with that data model.
 * `mermaid-compare.ts` runs it once per revision in separate processes.
 */

import { readFileSync } from "node:fs";
import { basename, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const U = 1;
const D = 2;
const L = 4;
const R = 8;
const HEADS: Record<string, [number, number]> = {
  "▼": [0, 1],
  "▽": [0, 1],
  "▲": [0, -1],
  "△": [0, -1],
  "◄": [-1, 0],
  "◁": [-1, 0],
  "▶": [1, 0],
  "▷": [1, 0],
};
/** Direction bit, neighbour offset, and the bit the neighbour must answer with. */
const LINKS: [number, number, number, number][] = [
  [U, 0, -1, D],
  [D, 0, 1, U],
  [L, -1, 0, R],
  [R, 1, 0, L],
];
const EDGE_GLYPHS = new Set(
  `─│┌┐└┘├┤┬┴┼╌╎━┃┏┓┗┛┣┫┳┻╋╭╮╰╯═║╪╤╧╫╟╢●o×◆◇${Object.keys(HEADS).join("")}`,
);

/** The `Canvas` fields the metrics read; structurally typed so any revision fits. */
interface CanvasLike {
  w: number;
  h: number;
  ch: string[];
  role: string[];
  mask: Uint8Array;
  occupied: Uint8Array;
  /** Edges through each cell and where labels landed; absent on old revisions. */
  owners?: Map<number, Set<number>>;
  written?: { row: number; x: number; w: number; edge: number; cut: boolean }[];
}

export interface Metrics {
  width: number;
  height: number;
  area: number;
  /** Two edges passing through one cell, drawn as a hop (`╫`). */
  crossings: number;
  /** Three or more arms meeting: a fork or join, drawn as a dot (`●`). */
  junctions: number;
  bends: number;
  /** Cells carrying edge bits. */
  routedLength: number;
  /** Columns right of the rightmost box (TD lanes live here). */
  marginRight: number;
  /** Rows below the lowest box (LR lanes live here). */
  marginBottom: number;
  /**
   * Hard failure: an edge cell whose direction bit points at a neighbour
   * that neither continues the line, is a box, an arrowhead, nor a label
   * written over the line — the gap a route leaves where a box or the
   * canvas edge swallowed it.
   */
  brokenLinks: number;
  /** Hard failure: edge bits under a label character. */
  edgeOverText: number;
  /** Hard failure: an arrowhead not pointing at a box border. */
  danglingHeads: number;
  /**
   * Hard failure: a line running into a box border that does not tee it
   * in — it goes on behind the box, where it cannot be followed at all.
   */
  behindBox: number;
  /** Hard failure: two edge labels touching on a row, read as one word. */
  labelGlued: number;
  /**
   * Hard failure: an edge label touching only other edges' lines, so it
   * reads as naming one of them.
   */
  labelAstray: number;
  /** Edge labels that lost part of their text for lack of room. */
  labelCut: number;
  /** Edge labels written across another edge's line. */
  labelOverLine: number;
  heads: number;
  ms: number;
}

const at = (c: CanvasLike, x: number, y: number): number | null =>
  x >= 0 && y >= 0 && x < c.w && y < c.h ? y * c.w + x : null;

/** Double-line borders carry no direction bits; a line tees into them by glyph. */
const DOUBLE = new Set("═║╔╗╚╝╤╧╟╢╪╫");

/**
 * Direction bits at an edge cell that point into a box whose border does
 * not answer: the line goes on behind the box instead of teeing into it.
 */
function behindAt(c: CanvasLike, x: number, y: number, mask: number): number {
  let behind = 0;
  for (const [bit, dx, dy, back] of LINKS) {
    const j = (mask & bit) === 0 ? null : at(c, x + dx, y + dy);
    if (j !== null && c.occupied[j] && (c.mask[j] & back) === 0 && !DOUBLE.has(c.ch[j])) behind++;
  }
  return behind;
}

/** Direction bits at cell `i` that no neighbour answers. */
function brokenAt(c: CanvasLike, x: number, y: number, mask: number): number {
  let broken = 0;
  for (const [bit, dx, dy, back] of LINKS) {
    if ((mask & bit) === 0) continue;
    const j = at(c, x + dx, y + dy);
    if (j === null) broken++;
    else if (
      (c.mask[j] & back) === 0 &&
      !c.occupied[j] &&
      HEADS[c.ch[j]] === undefined &&
      c.role[j] !== "edgeLabel"
    )
      broken++;
  }
  return broken;
}

/**
 * A head must point at a box border or at a junction dot it feeds. One
 * drawn in a border (the tail end of a two-headed edge) is attached
 * already: on a frame it points at the open inside.
 */
function headDangling(c: CanvasLike, x: number, y: number, dir: [number, number]): boolean {
  if (c.occupied[y * c.w + x]) return false;
  const j = at(c, x + dir[0], y + dir[1]);
  return j === null || (!c.occupied[j] && c.ch[j] !== "●");
}

/**
 * A label names a line it touches: one it is written into, the row above
 * or below it, or up to two blank cells beside it on its row (a self-loop's
 * label hangs two cells off its hook). Astray when it
 * touches lines and none of them is its own edge's. `over` is true when it
 * is written across another edge's line, cutting it.
 */
function labelPlace(
  c: CanvasLike,
  l: { row: number; x: number; w: number; edge: number },
): { astray: boolean; over: boolean } {
  const owners = (x: number, y: number): Set<number> | undefined =>
    at(c, x, y) === null ? undefined : c.owners?.get(y * c.w + x);
  const span = (from: number, to: number, y: number): Set<number>[] =>
    Array.from({ length: to - from }, (_, k) => owners(from + k, y)).filter((o) => o !== undefined);
  const inside = span(l.x, l.x + l.w, l.row);
  const touched = [
    ...span(l.x - 3, l.x + l.w + 3, l.row),
    ...span(l.x, l.x + l.w, l.row - 1),
    ...span(l.x, l.x + l.w, l.row + 1),
  ];
  const foreign = (o: Set<number>): boolean => !o.has(l.edge);
  return { astray: touched.length > 0 && touched.every(foreign), over: inside.some(foreign) };
}

const isBend = (mask: number): boolean =>
  mask === (D | R) || mask === (D | L) || mask === (U | R) || mask === (U | L);

function measureEdgeCell(c: CanvasLike, x: number, y: number, m: Metrics): void {
  const i = y * c.w + x;
  const mask = c.mask[i];
  const ch = c.ch[i];
  if (mask !== 0) {
    m.routedLength++;
    if (ch === "╫") m.crossings++;
    else if (ch === "●") m.junctions++;
    else if (isBend(mask)) m.bends++;
    if (!EDGE_GLYPHS.has(ch) && ch !== "\0") m.edgeOverText++;
    m.brokenLinks += brokenAt(c, x, y, mask);
    if (!c.occupied[y * c.w + x]) m.behindBox += behindAt(c, x, y, mask);
  }
  const dir = HEADS[ch];
  if (dir !== undefined) {
    m.heads++;
    if (headDangling(c, x, y, dir)) m.danglingHeads++;
  }
}

export function measure(c: CanvasLike, ms: number): Metrics {
  const m: Metrics = {
    width: c.w,
    height: c.h,
    area: c.w * c.h,
    crossings: 0,
    junctions: 0,
    bends: 0,
    routedLength: 0,
    marginRight: 0,
    marginBottom: 0,
    brokenLinks: 0,
    edgeOverText: 0,
    danglingHeads: 0,
    behindBox: 0,
    labelGlued: 0,
    labelAstray: 0,
    labelCut: 0,
    labelOverLine: 0,
    heads: 0,
    ms,
  };
  let maxBoxX = -1;
  let maxBoxY = -1;
  for (let y = 0; y < c.h; y++) {
    for (let x = 0; x < c.w; x++) {
      const i = y * c.w + x;
      if (c.occupied[i]) {
        maxBoxX = Math.max(maxBoxX, x);
        maxBoxY = Math.max(maxBoxY, y);
      }
      if (c.role[i] === "edge") measureEdgeCell(c, x, y, m);
    }
  }
  const written = (c.written ?? []).filter((l) => l.w > 0);
  for (const l of written) {
    if (written.some((o) => o !== l && o.row === l.row && (o.x === l.x + l.w || o.x + o.w === l.x)))
      m.labelGlued++;
    const place = labelPlace(c, l);
    // A label with no room for its text is cut wherever it fits: counted
    // as cut, not as astray.
    if (l.cut) m.labelCut++;
    else if (place.astray) m.labelAstray++;
    if (place.over) m.labelOverLine++;
  }
  m.marginRight = c.w - 1 - maxBoxX;
  m.marginBottom = c.h - 1 - maxBoxY;
  return m;
}

export const HARD_KEYS = [
  "brokenLinks",
  "edgeOverText",
  "danglingHeads",
  "behindBox",
  "labelGlued",
  "labelAstray",
] as const;

export interface FixtureResult {
  name: string;
  plain: string[] | null;
  metrics: Metrics | null;
  deterministic: boolean;
}

type Limits = { wrap: number; lines: number; label: number };
/** The renderer's default label limits; the baseline renderer ignores the argument. */
const DEFAULT_LIMITS: Limits = { wrap: 24, lines: 4, label: 28 };

interface Registry {
  diagramFor(src: string): {
    render(
      src: string,
      limits: Limits,
    ): { canvas: CanvasLike & { toLines(): { plain: string[] } } } | null;
  } | null;
}

export async function loadRegistry(root: string): Promise<Registry> {
  const file = resolve(root, "plugins/pi-loom-mermaid/src/loom-mermaid/registry.ts");
  return (await import(pathToFileURL(file).href)) as Registry;
}

export function renderFixture(registry: Registry, name: string, src: string): FixtureResult {
  const draw = (): { canvas: CanvasLike & { toLines(): { plain: string[] } } } | null =>
    registry.diagramFor(src)?.render(src, DEFAULT_LIMITS) ?? null;
  const t0 = performance.now();
  const first = draw();
  const ms = performance.now() - t0;
  if (first === null) return { name, plain: null, metrics: null, deterministic: true };
  const plain = first.canvas.toLines().plain;
  const again = draw()?.canvas.toLines().plain ?? [];
  return {
    name,
    plain,
    metrics: measure(first.canvas, ms),
    deterministic: plain.join("\n") === again.join("\n"),
  };
}

async function main(): Promise<void> {
  const [root, ...fixtures] = process.argv.slice(2);
  if (!root || fixtures.length === 0) {
    console.error("usage: mermaid-metrics.ts <root> <fixture.mmd>...");
    process.exit(2);
  }
  const registry = await loadRegistry(root);
  const results = fixtures.map((f) =>
    renderFixture(registry, basename(f, ".mmd"), readFileSync(f, "utf8")),
  );
  process.stdout.write(JSON.stringify(results));
}

if (process.argv[1] && resolve(process.argv[1]) === new URL(import.meta.url).pathname) {
  await main();
}
