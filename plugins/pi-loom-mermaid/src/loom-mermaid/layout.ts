/**
 * Graph layout: rank, order, place, route, draw.
 *
 * Follows the Sugiyama outline — assign ranks along the flow axis, reorder
 * within ranks to cut crossings, then relax positions on the cross axis so
 * chains stay straight. Edges between adjacent ranks share horizontal "bus"
 * rows; everything else is routed around the diagram through vertical "lanes".
 *
 * `BT` and `RL` reuse the `TD`/`LR` layouts and flip the finished canvas, so
 * text never ends up mirrored.
 */

import {
  Canvas,
  CONT,
  D,
  drawText,
  drawTextOverEdges,
  L,
  R,
  STY_DOT,
  STY_SOLID,
  STY_THICK,
  U,
} from './canvas.ts'
import type { Edge, Head, Node, Shape } from './graph.ts'
import { Graph } from './graph.ts'
import { fitLabel, MAX_LABEL, MAX_LINES, WRAP_WIDTH, wrapLabel } from './labels.ts'
import { brandesKoepf, type LayeredGraph } from './placement.ts'
import { measured, stringWidth } from './width.ts'

/** Cells of padding between a box border and its text. */
export const PAD = 1
/** Minimum horizontal / vertical space between boxes. */
const GAP_X = 3
const GAP_Y = 2
/** Refuse to allocate a canvas larger than this many cells. */
export const MAX_CANVAS_CELLS = 1 << 21

/** A laid-out canvas, or `null` when the diagram is empty or over the cell cap. */
export type CanvasResult = Canvas | null

/** Saturating subtraction; Rust's `usize` arithmetic never goes negative. */
export const sat = (a: number, b: number): number => Math.max(0, a - b)
export const half = (n: number): number => Math.floor(n / 2)

/**
 * Everything an edge says, joined — the fallback for routes that have no
 * per-end placement (lanes, self-loops). Forward routes place `cardFrom` /
 * `cardTo` at their own ends instead.
 */
function edgeText(edge: Edge): string | null {
  const joined = [edge.cardFrom ?? '', edge.label ?? '', edge.cardTo ?? '']
    .filter((part) => part !== '')
    .join(' ')
  return joined === '' ? null : joined
}

export interface Placed {
  x: number
  y: number
  w: number
  h: number
  cx: number
  cy: number
  rank: number
}

/** Per-node dimensions. `lay*` include room for self-edge loops and labels. */
interface NodeSizes {
  boxW: number[]
  boxH: number[]
  layW: number[]
  layH: number[]
  extraH: number[]
  selfLabelW: number[]
}

/** What to draw inside a node box. */
export type NodeExtra =
  | { kind: 'plain' }
  | { kind: 'frame'; sub: Canvas }
  | { kind: 'compartments'; sections: string[][] }

interface RoutePlan {
  canvasW: number
  canvasH: number
  /** Coordinate just past each rank's boxes, where its bus rows begin. */
  bandEnd: number[]
  /** Coordinate where each rank's boxes begin — no box sits before it. */
  rankStart: number[]
  /** Bus track offset per edge. */
  edgeBus: number[]
  /** Coordinate of the first lane track. */
  laneBase: number
  /** Lane track offset per edge. */
  edgeLane: number[]
  /** Skip edges: the column entering the target's top, or -1. */
  edgeEntryX: number[]
  /** Skip edges routed through the interior along their virtual chain. */
  edgeStraight: boolean[]
  /**
   * Interior skip route: each jog's bus coordinate and the cross-axis
   * coordinate the edge continues along after it, in flow order.
   */
  skipRoute: { bus: number; at: number }[][]
  /** Per node: the forward cluster's entry column, or -1 for the centre. */
  fwdEntryX: number[]
  /** Per edge: render the label left of the arrowhead instead of right. */
  edgeLabelLeft: boolean[]
}

// ------------------------------------------------------------------ ranking

/**
 * Longest-path ranking over the graph's DAG.
 *
 * Back edges (those closing a cycle) are excluded by a DFS colouring pass, so
 * `A --> B --> C --> A` still ranks 0, 1, 2 rather than diverging.
 */
export function computeRanks(graph: Graph): number[] {
  const n = graph.nodes.length
  const children: number[][] = Array.from({ length: n }, () => [])
  const indeg = new Array<number>(n).fill(0)
  for (const e of graph.edges) {
    if (e.from !== e.to) {
      children[e.from].push(e.to)
      indeg[e.to]++
    }
  }

  const color = new Uint8Array(n)
  const dag: number[][] = Array.from({ length: n }, () => [])
  const order: number[] = []

  // Roots first so ranks grow from natural entry points, then any leftovers.
  const roots = [...Array(n).keys()].filter((i) => indeg[i] === 0)
  for (const start of [...roots, ...Array(n).keys()]) {
    if (color[start] === 0) dfsDag(start, children, color, dag, order)
  }

  const rank = new Array<number>(n).fill(0)
  for (let i = order.length - 1; i >= 0; i--) {
    const u = order[i]
    for (const v of dag[u]) rank[v] = Math.max(rank[v], rank[u] + 1)
  }
  return rank
}

/** Iterative DFS recording postorder and skipping edges back into the stack. */
function dfsDag(
  start: number,
  children: number[][],
  color: Uint8Array,
  dag: number[][],
  order: number[],
): void {
  const stack: { u: number; i: number }[] = [{ u: start, i: 0 }]
  color[start] = 1
  while (stack.length > 0) {
    const frame = stack[stack.length - 1]
    const u = frame.u
    if (frame.i < children[u].length) {
      const v = children[u][frame.i]
      frame.i++
      if (color[v] === 1) continue // grey: a back edge, ignore it
      dag[u].push(v)
      if (color[v] === 0) {
        color[v] = 1
        stack.push({ u: v, i: 0 })
      }
    } else {
      color[u] = 2
      order.push(u)
      stack.pop()
    }
  }
}

/**
 * The layered graph crossing reduction works on: every real node plus one
 * virtual node per intermediate rank of each forward edge spanning more than
 * one rank (the edge becomes a chain of unit segments). Ids below `n` are
 * real; `up[id]` / `down[id]` list unit-segment neighbours.
 */
export interface Layered extends LayeredGraph {
  /** Per edge, its virtual nodes from source to target; empty unless it skips ranks. */
  chains: number[][]
}

/**
 * Split each forward edge into unit-rank segments. Naive normalisation is
 * bounded by MAX_EDGES × MAX_NODES virtual nodes, small enough here.
 */
function normalize(byRank: number[][], edges: Edge[], ranks: number[]): Layered {
  const n = ranks.length
  const layers = byRank.map((row) => [...row])
  const up: number[][] = Array.from({ length: n }, () => [])
  const down: number[][] = Array.from({ length: n }, () => [])
  const link = (a: number, b: number): void => {
    down[a].push(b)
    up[b].push(a)
  }
  const chains: number[][] = edges.map(() => [])
  edges.forEach((e, i) => {
    if (e.from === e.to || ranks[e.to] <= ranks[e.from]) return
    let prev = e.from
    for (let r = ranks[e.from] + 1; r < ranks[e.to]; r++) {
      const v = up.length
      up.push([])
      down.push([])
      layers[r].push(v)
      chains[i].push(v)
      link(prev, v)
      prev = v
    }
    link(prev, e.to)
  })
  return { layers, up, down, chains }
}

/**
 * Reorder nodes within each rank to minimise edge crossings.
 *
 * Long edges are normalised into virtual-node chains first, so every
 * boundary crossing is visible to the count and a long edge is ordered as
 * one coherent chain. Alternate down/up barycenter sweeps are each followed
 * by adjacent-transposition cleanup; sweeping stops after two rounds without
 * improvement, keeping whichever ordering crossed least.
 *
 * `trailing` nodes must end their rank (lane endpoints: the strip they exit
 * toward lies past the rank's last box, so anything ordered beyond them
 * would be cut through). The constraint is applied inside every sweep, so the
 * crossing count that picks the best order is the count of the order used.
 */
export function orderRanks(
  byRank: number[][],
  edges: Edge[],
  ranks: number[],
  trailing: boolean[] = [],
): Layered {
  const n = ranks.length
  const isTrailing = (v: number): boolean => trailing[v] ?? false
  const partition = (row: number[]): void => {
    row.sort((a, b) => Number(isTrailing(a)) - Number(isTrailing(b)))
  }
  for (const row of byRank) partition(row)
  const layered = normalize(byRank, edges, ranks)
  if (byRank.length < 2 || n < 3) return layered

  const { layers, up, down } = layered
  const pos = new Array<number>(up.length).fill(0)
  const reindex = (row: number[]): void => {
    for (let i = 0; i < row.length; i++) pos[row[i]] = i
  }
  for (const row of layers) reindex(row)
  const total = (): number => {
    let sum = 0
    for (let r = 0; r + 1 < layers.length; r++) sum += crossingsBetween(layers[r], down, pos)
    return sum
  }

  let best = layers.map((row) => [...row])
  let bestCrossings = total()
  let stale = 0
  for (let it = 0; bestCrossings > 0 && stale < 2 && it < 24; it++) {
    const downward = it % 2 === 0
    const rows = downward ? layers.slice(1) : layers.slice(0, -1).reverse()
    const neigh = downward ? up : down
    for (const row of rows) {
      sortByBarycenter(row, neigh, pos)
      partition(row)
      reindex(row)
    }
    transpose(layers, up, down, pos, isTrailing)
    const crossings = total()
    if (crossings < bestCrossings) {
      bestCrossings = crossings
      best = layers.map((row) => [...row])
      stale = 0
    } else stale++
  }

  for (let i = 0; i < byRank.length; i++) {
    byRank[i].splice(0, byRank[i].length, ...best[i].filter((v) => v < n))
  }
  return { ...layered, layers: best }
}

function sortByBarycenter(row: number[], neigh: number[][], pos: number[]): void {
  const key = (v: number): number =>
    neigh[v].length === 0 ? pos[v] : neigh[v].reduce((s, u) => s + pos[u], 0) / neigh[v].length
  const keyed = row.map((v) => ({ key: key(v), v }))
  keyed.sort((a, b) => a.key - b.key)
  for (let i = 0; i < keyed.length; i++) row[i] = keyed[i].v
}

/**
 * Swap adjacent nodes while that lowers the crossings with both neighbouring
 * layers (Gansner et al.'s transpose step). Never swaps across the
 * trailing boundary.
 */
function transpose(
  layers: number[][],
  up: number[][],
  down: number[][],
  pos: number[],
  isTrailing: (v: number) => boolean,
): void {
  let improved = true
  for (let guard = 0; improved && guard < 8; guard++) {
    improved = false
    for (const row of layers) {
      for (let i = 0; i + 1 < row.length; i++) {
        const v = row[i]
        const w = row[i + 1]
        if (isTrailing(v) !== isTrailing(w)) continue
        const before = pairCrossings(v, w, up, pos) + pairCrossings(v, w, down, pos)
        const after = pairCrossings(w, v, up, pos) + pairCrossings(w, v, down, pos)
        if (after < before) {
          row[i] = w
          row[i + 1] = v
          pos[w] = i
          pos[v] = i + 1
          improved = true
        }
      }
    }
  }
}

/** Crossings among the segments of `v` and `w` if `v` sits left of `w`. */
function pairCrossings(v: number, w: number, neigh: number[][], pos: number[]): number {
  let count = 0
  for (const a of neigh[v]) for (const b of neigh[w]) if (pos[a] > pos[b]) count++
  return count
}

/**
 * Crossings between `row` and the layer below it: segments sorted by their
 * upper end, then inversions of the lower ends counted with a Fenwick tree
 * (Barth, Mutzel and Jünger's O(M log N) method).
 */
function crossingsBetween(row: number[], down: number[][], pos: number[]): number {
  const lower: number[] = []
  let width = 0
  for (const v of row) {
    const ends = down[v].map((u) => pos[u]).sort((a, b) => a - b)
    for (const p of ends) {
      lower.push(p)
      width = Math.max(width, p + 1)
    }
  }
  const tree = new Array<number>(width + 1).fill(0)
  let crossings = 0
  for (let i = 0; i < lower.length; i++) {
    // Earlier segments ending right of this one cross it.
    let greater = i
    for (let k = lower[i] + 1; k > 0; k -= k & -k) greater -= tree[k]
    crossings += greater
    for (let k = lower[i] + 1; k <= width; k += k & -k) tree[k]++
  }
  return crossings
}

/**
 * Cross-axis centre for every node of the layered graph, real and virtual:
 * Brandes–Köpf over measured sizes. A virtual chain node takes one cell so
 * the long edge it carries has a clear column (row, in LR) to run along,
 * one blank cell from whatever neighbours it — plus `pad(left)` cells when
 * it follows a real node, room for that node's arrival labels.
 */
export function assignPositions(
  layered: Layered,
  size: number[],
  sep: number,
  pad: (node: number) => number = () => 0,
): number[] {
  const n = size.length
  const all = [...size]
  while (all.length < layered.up.length) all.push(1)
  const sepOf = (left: number, right: number): number =>
    left < n && right < n ? sep : 1 + (left < n && right >= n ? pad(left) : 0)
  return brandesKoepf(layered, all, sepOf, n)
}

// ------------------------------------------------------------------- tracks

/** A span competing for a track: the covered coordinate range plus its edge. */
interface TrackSpan {
  start: number
  end: number
  from: number
  to: number
  edge: number
  /** A labelled lane refuses endpoint sharing: the label would appear to
   * cover every edge merged onto the row. Bus spans never set this — their
   * labels sit at the separate arrival ends, so fan merging stays safe. */
  labeled?: boolean
}

/**
 * Pack spans into as few parallel tracks as possible.
 *
 * Two spans share a track when they are two cells apart, or when they share an
 * endpoint — edges fanning out of one node deliberately reuse a single row so
 * a merge draws one arrowhead rather than a stack of them.
 */
export function assignTracks(
  spans: TrackSpan[],
  shortestFirst = false,
): { assigned: [number, number][]; count: number } {
  // Lanes pack shortest-first: a span contained in another takes the inner
  // track, so exits and entries at rows the inner lane never reaches cross
  // nothing. Buses keep the start-ordered packing.
  const sorted = [...spans].sort(
    (a, b) =>
      (shortestFirst ? a.end - a.start - (b.end - b.start) : 0) ||
      a.start - b.start ||
      a.end - b.end ||
      a.from - b.from ||
      a.to - b.to ||
      a.edge - b.edge,
  )
  const tracks: TrackSpan[][] = []
  const assigned: [number, number][] = []
  for (const span of sorted) {
    let slot = tracks.findIndex((members) =>
      members.every(
        (m) =>
          m.end + 2 <= span.start ||
          span.end + 2 <= m.start ||
          ((m.from === span.from || m.to === span.to) && !m.labeled && !span.labeled),
      ),
    )
    if (slot === -1) {
      tracks.push([])
      slot = tracks.length - 1
    }
    tracks[slot].push(span)
    assigned.push([span.edge, slot])
  }
  return { assigned, count: tracks.length }
}

/**
 * An edge between adjacent ranks running against the direction. In top-down
 * layouts these route locally through the same band as their forward
 * siblings — the short return arrow mermaid draws — rather than around the
 * diagram. Left-to-right boxes are three rows tall, leaving no room to
 * offset the return off the centre row, so LR/RL keep the lane.
 */
export const isAdjacentBack = (ranks: number[], e: Edge): boolean =>
  e.from !== e.to && ranks[e.from] === ranks[e.to] + 1

/** Edges crossing the band between rank `r` and `r + 1` that must jog
 * sideways, so need a bus row. `withBack` admits adjacent back edges. */
function busSpans(
  graph: Graph,
  ranks: number[],
  centers: number[],
  r: number,
  exact: boolean,
  withBack: boolean,
): TrackSpan[] {
  const out: TrackSpan[] = []
  graph.edges.forEach((e, i) => {
    const jogs = exact
      ? centers[e.from] !== centers[e.to]
      : Math.abs(centers[e.from] - centers[e.to]) > 1
    const lo = Math.min(ranks[e.from], ranks[e.to])
    const adjacent = withBack
      ? Math.abs(ranks[e.from] - ranks[e.to]) === 1
      : ranks[e.to] === ranks[e.from] + 1
    if (e.from !== e.to && adjacent && lo === r && jogs) {
      out.push({
        start: Math.min(centers[e.from], centers[e.to]),
        end: Math.max(centers[e.from], centers[e.to]),
        from: e.from,
        to: e.to,
        edge: i,
      })
    }
  })
  return out
}

/** Edges skipping a rank or running backwards; these go around in a lane. */
function laneSpans(
  graph: Graph,
  ranks: number[],
  placed: Placed[],
  vertical: boolean,
): TrackSpan[] {
  const out: TrackSpan[] = []
  graph.edges.forEach((e, i) => {
    if (e.from === e.to || ranks[e.to] === ranks[e.from] + 1) return
    if (vertical && isAdjacentBack(ranks, e)) return
    const pf = placed[e.from]
    const pt = placed[e.to]
    const a = vertical ? Math.min(pf.cy, pt.cy) : Math.min(pf.cx, pt.cx)
    const b = vertical ? Math.max(pf.cy, pt.cy) : Math.max(pf.cx, pt.cx)
    out.push({ start: a, end: b, from: e.from, to: e.to, edge: i, labeled: edgeText(e) !== null })
  })
  return out
}

// ----------------------------------------------------------------- placement

/** One sideways jog of an interior skip route, competing for a bus track. */
interface ChainJog extends TrackSpan {
  band: number
  /** Cross-axis coordinate the edge continues along after the jog. */
  at: number
}

/**
 * The jogs a skip edge makes following its virtual chain: source centre to
 * the first chain coordinate, between chain nodes where they differ, and
 * from the last one to the entry coordinate. Edges whose chain is empty
 * (`entry[i] === -1` opts an edge out) get none and stay on the lane.
 */
function chainJogs(
  graph: Graph,
  ranks: number[],
  layered: Layered,
  centers: number[],
  entry: number[],
  isSkip: (e: Edge) => boolean,
): ChainJog[] {
  const jogs: ChainJog[] = []
  graph.edges.forEach((e, i) => {
    const chain = layered.chains[i]
    if (!isSkip(e) || chain.length === 0 || entry[i] === -1) return
    const stops = [centers[e.from], ...chain.map((v) => centers[v]), entry[i]]
    const ids = [e.from, ...chain, e.to]
    for (let k = 0; k + 1 < stops.length; k++) {
      if (stops[k] === stops[k + 1]) continue
      jogs.push({
        band: ranks[e.from] + k,
        at: stops[k + 1],
        start: Math.min(stops[k], stops[k + 1]),
        end: Math.max(stops[k], stops[k + 1]),
        from: ids[k],
        to: ids[k + 1],
        edge: i,
      })
    }
  })
  return jogs
}

/** Per edge, its jogs as route waypoints once bus coordinates are known. */
function skipRoutes(
  graph: Graph,
  jogs: ChainJog[],
  busOf: (j: ChainJog) => number,
): { bus: number; at: number }[][] {
  const routes: { bus: number; at: number }[][] = graph.edges.map(() => [])
  for (const j of jogs) routes[j.edge].push({ bus: busOf(j), at: j.at })
  return routes
}

function placeTd(
  ranks: number[],
  maxRank: number,
  byRank: number[][],
  layered: Layered,
  sizes: NodeSizes,
  graph: Graph,
  placed: Placed[],
): RoutePlan {
  // Arrival labels hang right of a box's entry heads, on the row a chain
  // column passes through: a chain placed right of a box keeps clear of them.
  const labelPad = new Array<number>(graph.nodes.length).fill(0)
  for (const e of graph.edges) {
    if (e.from === e.to || ranks[e.to] <= ranks[e.from]) continue
    for (const part of [e.label, e.cardTo]) {
      if (part != null) labelPad[e.to] = Math.max(labelPad[e.to], Math.min(stringWidth(part), MAX_LABEL))
    }
  }
  const centers = assignPositions(layered, sizes.layW, GAP_X, (node) => labelPad[node])

  // Top-entry geometry, derivable before placement. A node's entries — the
  // merged forward cluster plus each skip — spread evenly across the box top
  // (two entries land at ~1/3 and ~2/3). A forward arrival aligned with its
  // source stays at centre so chains keep no kinks, and the skips spread
  // over the right half instead. A label that does not fit before the next
  // entry renders left of its arrow. A skip whose entry column crosses no
  // box on any intermediate rank drops straight instead of taking a lane.
  const boxL = (j: number): number => sat(centers[j], half(sizes.boxW[j]))
  const boxR = (j: number): number => boxL(j) + sizes.boxW[j] - 1
  const isSkip = (e: Edge): boolean => e.from !== e.to && ranks[e.to] - ranks[e.from] > 1
  const edgeEntryX = new Array<number>(graph.edges.length).fill(-1)
  const edgeStraight = new Array<boolean>(graph.edges.length).fill(false)
  const fwdEntryX = new Array<number>(graph.nodes.length).fill(-1)
  const edgeLabelLeft = new Array<boolean>(graph.edges.length).fill(false)
  const labelW = (e: Edge): number => {
    const parts = [e.label, e.cardTo].filter((p) => p != null) as string[]
    return parts.length === 0
      ? -1
      : Math.max(...parts.map((p) => Math.min(stringWidth(p), MAX_LABEL)))
  }
  const skipsInto: number[][] = graph.nodes.map(() => [])
  const fwdsInto: number[][] = graph.nodes.map(() => [])
  graph.edges.forEach((e, i) => {
    if (e.from === e.to) return
    if (isSkip(e)) skipsInto[e.to].push(i)
    else if (ranks[e.to] === ranks[e.from] + 1) fwdsInto[e.to].push(i)
  })
  graph.nodes.forEach((_, t) => {
    const skips = skipsInto[t]
    if (skips.length === 0) return
    const fwds = fwdsInto[t]
    const cx = centers[t]
    const left = boxL(t)
    const right = boxR(t)
    const hasFwd = fwds.length > 0
    const aligned = hasFwd && fwds.some((i) => Math.abs(centers[graph.edges[i].from] - cx) <= 1)
    const spreadL = hasFwd && aligned ? cx : left
    const slots = hasFwd && aligned ? skips.length : (hasFwd ? 1 : 0) + skips.length
    const slot = (i: number): number =>
      spreadL + Math.round(((right - spreadL) * (i + 1)) / (slots + 1))
    /** Ordered entries: the forward cluster (if any), then each skip. */
    const items: { slot: number; w: number; skip: number | null }[] = []
    if (hasFwd) {
      items.push({
        slot: aligned ? cx : slot(0),
        w: Math.max(...fwds.map((i) => labelW(graph.edges[i]))),
        skip: null,
      })
    }
    skips.forEach((si, j) => {
      items.push({
        slot: slot(hasFwd && aligned ? j : j + (hasFwd ? 1 : 0)),
        w: labelW(graph.edges[si]),
        skip: si,
      })
    })
    // Walk left to right with a cursor over the free head-row cells: each
    // entry lands at its slot (or past the previous label), its own label
    // going right when the next slot leaves room, else left when the cells
    // behind the cursor allow. Overflow falls back to the legacy rule.
    const cols: number[] = []
    const lefts: boolean[] = []
    let cursor = left
    let fits = true
    for (const [i, item] of items.entries()) {
      const x = Math.max(item.slot, cursor)
      if (x > right - 1) {
        fits = false
        break
      }
      const next = items[i + 1]?.slot ?? Number.MAX_SAFE_INTEGER
      const w = item.w
      if (w >= 0 && x + w + 2 > next && x - cursor >= w) {
        lefts.push(true)
        cursor = x + 2
      } else {
        lefts.push(false)
        cursor = w >= 0 ? x + w + 2 : x + 2
      }
      cols.push(x)
    }
    if (fits) {
      items.forEach((item, i) => {
        if (item.skip === null) {
          fwdEntryX[t] = cols[i]
          if (lefts[i]) for (const fi of fwds) edgeLabelLeft[fi] = true
        } else {
          edgeEntryX[item.skip] = cols[i]
          edgeLabelLeft[item.skip] = lefts[i]
        }
      })
      return
    }
    // Legacy: forward stays centred; a skip lands past the arrival labels,
    // or left of centre with its own label flipped left.
    const reach = hasFwd ? Math.max(cx, ...fwds.map((i) => cx + 1 + labelW(graph.edges[i]))) : -1
    for (const si of skips) {
      const clear = reach === -1 ? cx + 2 : reach + 2
      const capped = Math.min(clear, right - 1)
      if (capped > Math.max(cx, reach)) {
        edgeEntryX[si] = capped
      } else {
        edgeEntryX[si] = Math.max(left + 1, cx - 2)
        edgeLabelLeft[si] = true
      }
    }
  })
  // Every skip runs through the interior along the column its virtual chain
  // reserved; each band it jogs in lends it a bus track. The departure jog
  // shares the source's fan row (endpoint sharing), so a node's forward fan
  // and its skips split from one `┴` origin.
  const jogs = chainJogs(graph, ranks, layered, centers, edgeEntryX, isSkip)
  for (const j of jogs) edgeStraight[j.edge] = true
  const jogTrack = new Map<ChainJog, number>()

  const edgeBus = new Array<number>(graph.edges.length).fill(0)
  const busTracks = new Array<number>(maxRank + 1).fill(0)
  for (let r = 0; r < maxRank; r++) {
    const spans = busSpans(graph, ranks, centers, r, false, true)
    const bandJogs = jogs.filter((j) => j.band === r)
    spans.push(...bandJogs)
    if (spans.length === 0) continue
    // Back-edge arrowheads sit on the first band row, back buses right under
    // it, forward buses below those: with the attach columns offset right of
    // centre, a reciprocal pair then runs as two parallel staircases whose
    // verticals fall outside each other's horizontal spans — no crossings.
    const back = spans.filter((s) => isAdjacentBack(ranks, graph.edges[s.edge]))
    const fwd = spans.filter((s) => !isAdjacentBack(ranks, graph.edges[s.edge]))
    const base = graph.edges.some((e) => isAdjacentBack(ranks, e) && ranks[e.to] === r) ? 1 : 0
    const b = assignTracks(back)
    for (const [idx, slot] of b.assigned) edgeBus[idx] = base + slot
    const f = assignTracks(fwd)
    for (const [idx, slot] of f.assigned) edgeBus[idx] = base + b.count + slot
    for (const j of bandJogs) jogTrack.set(j, edgeBus[j.edge])
    busTracks[r] = base + b.count + f.count
  }

  const rankH = byRank.map((row) =>
    row.length === 0 ? 3 : Math.max(...row.map((i) => sizes.boxH[i] + sizes.extraH[i])),
  )
  // Per-end cardinalities want a row each around the verb: source card,
  // label, arrow-and-target-card.
  const hasCards = graph.edges.some((e) => e.cardFrom !== undefined || e.cardTo !== undefined)
  const gapY = hasCards ? Math.max(GAP_Y, 3) : GAP_Y
  const rankY = new Array<number>(maxRank + 1).fill(0)
  for (let r = 1; r <= maxRank; r++) {
    rankY[r] = rankY[r - 1] + rankH[r - 1] + Math.max(gapY, busTracks[r - 1] + 1)
  }
  const canvasH = rankY[maxRank] + rankH[maxRank]
  const bandEnd = Array.from({ length: maxRank + 1 }, (_, r) => rankY[r] + rankH[r])
  const rankStart = rankY
  const skipRoute = skipRoutes(graph, jogs, (j) => bandEnd[j.band] + (jogTrack.get(j) ?? 0))

  let diagramW = 1
  for (let v = graph.nodes.length; v < centers.length; v++) diagramW = Math.max(diagramW, centers[v] + 1)
  byRank.forEach((row, r) => {
    for (const idx of row) {
      const w = sizes.boxW[idx]
      const h = sizes.boxH[idx]
      const cx = centers[idx]
      const x = sat(cx, half(w))
      const y = rankY[r] + half(rankH[r] - h - sizes.extraH[idx])
      placed[idx] = { x, y, w, h, cx, cy: y + half(h), rank: r }
      diagramW = Math.max(diagramW, x + w)
      if (sizes.extraH[idx] > 0 && sizes.selfLabelW[idx] > 0) {
        diagramW = Math.max(diagramW, x + w + 2 + sizes.selfLabelW[idx])
      }
    }
  })

  let contentW = diagramW
  for (const e of graph.edges) {
    if (e.from === e.to) continue
    if (ranks[e.to] > ranks[e.from]) {
      const parts = [e.label, e.cardTo].filter((part) => part != null) as string[]
      const entry = Math.max(placed[e.to].cx, edgeEntryX[graph.edges.indexOf(e)])
      for (const part of parts) {
        const lw = Math.min(stringWidth(part), MAX_LABEL)
        contentW = Math.max(contentW, entry + 2 + lw)
      }
      if (e.cardFrom !== undefined) {
        contentW = Math.max(contentW, placed[e.from].cx + 2 + stringWidth(e.cardFrom))
      }
    } else if (isAdjacentBack(ranks, e)) {
      const text = edgeText(e)
      if (text !== null) {
        // routeBackAdjacent starts the label at tx + 1 with tx up to cx + 2.
        contentW = Math.max(contentW, placed[e.to].cx + 3 + Math.min(stringWidth(text), MAX_LABEL))
      }
    } else {
      const text = edgeText(e)
      if (text !== null) {
        contentW = Math.max(contentW, diagramW + Math.min(stringWidth(text), MAX_LABEL) + 1)
      }
    }
  }

  const edgeLane = new Array<number>(graph.edges.length).fill(0)
  const lanes = laneSpans(graph, ranks, placed, true).filter((s) => !edgeStraight[s.edge])
  let canvasW = contentW
  let laneBase = 0
  if (lanes.length > 0) {
    const { assigned, count } = assignTracks(lanes, true)
    for (const [idx, slot] of assigned) edgeLane[idx] = slot
    canvasW = contentW + 1 + count
    laneBase = contentW + 1
  }

  return {
    canvasW,
    canvasH,
    bandEnd,
    rankStart,
    edgeBus,
    laneBase,
    edgeLane,
    edgeEntryX,
    edgeStraight,
    skipRoute,
    fwdEntryX,
    edgeLabelLeft,
  }
}

function placeLr(
  ranks: number[],
  maxRank: number,
  byRank: number[][],
  layered: Layered,
  sizes: NodeSizes,
  graph: Graph,
  placed: Placed[],
): RoutePlan {
  const colW = byRank.map((row) =>
    row.length === 0 ? 0 : Math.max(...row.map((i) => sizes.boxW[i])),
  )

  const centers = assignPositions(layered, sizes.layH, 1)

  // A skip whose target entry row crosses no box on any intermediate rank
  // runs straight through the diagram into the target's left side, exiting
  // through the source's right-side fan; the bottom lane is the fallback.
  // (No entry spreading or local returns here: LR boxes are three rows tall,
  // so the centre row is the only usable port on a side.)
  const isSkip = (e: Edge): boolean => e.from !== e.to && ranks[e.to] - ranks[e.from] > 1
  // A back-edge target's bottom-entry `▲` stub sits one row below its box;
  // a straight run through that cell would appear to carry the arrival.
  const stubRows = new Set<number>()
  for (const e of graph.edges) {
    if (e.from === e.to || ranks[e.to] >= ranks[e.from]) continue
    const t = e.to
    stubRows.add(sat(centers[t], half(sizes.boxH[t] + sizes.extraH[t])) + sizes.boxH[t])
  }
  // A skip follows the row its virtual chain reserved into the target's
  // centre row, unless a chain row is a stub row — then the lane.
  const edgeStraight = new Array<boolean>(graph.edges.length).fill(false)
  const entryY = graph.edges.map((e, i) =>
    isSkip(e) && !layered.chains[i].some((v) => stubRows.has(centers[v])) ? centers[e.to] : -1,
  )
  const jogs = chainJogs(graph, ranks, layered, centers, entryY, isSkip)
  for (const j of jogs) edgeStraight[j.edge] = true
  graph.edges.forEach((e, i) => {
    if (isSkip(e) && entryY[i] !== -1) edgeStraight[i] = true
  })
  const jogTrack = new Map<ChainJog, number>()

  // Left-to-right edge labels sit in the gap after their source's column, so
  // each gap sizes to the widest label *leaving through it* — one long label
  // widens its own band, not the whole diagram. Straight skips label there
  // too; a self-loop's label hangs beside its own box (selfLabelW).
  const bandLabel = new Array<number>(maxRank + 1).fill(0)
  graph.edges.forEach((e, i) => {
    if (e.from === e.to) return
    if (ranks[e.to] !== ranks[e.from] + 1 && !edgeStraight[i]) return
    const verb = e.label === null ? 0 : Math.min(stringWidth(e.label), MAX_LABEL)
    const cards = [e.cardFrom, e.cardTo]
      .filter((c) => c !== undefined)
      .reduce((w, c) => w + stringWidth(c as string) + 1, 0)
    bandLabel[ranks[e.from]] = Math.max(bandLabel[ranks[e.from]], verb + cards)
  })

  const edgeBus = new Array<number>(graph.edges.length).fill(0)
  const busTracks = new Array<number>(maxRank + 1).fill(0)
  for (let r = 0; r < maxRank; r++) {
    const spans = busSpans(graph, ranks, centers, r, true, false)
    const bandJogs = jogs.filter((j) => j.band === r)
    spans.push(...bandJogs)
    if (spans.length === 0) continue
    const { assigned, count } = assignTracks(spans)
    for (const [idx, slot] of assigned) edgeBus[idx] = slot
    for (const j of bandJogs) jogTrack.set(j, edgeBus[j.edge])
    busTracks[r] = count
  }

  const rankX = new Array<number>(maxRank + 1).fill(0)
  for (let r = 1; r <= maxRank; r++) {
    const gap = Math.max(GAP_X + 1, bandLabel[r - 1] + 3, busTracks[r - 1] + 1)
    rankX[r] = rankX[r - 1] + colW[r - 1] + gap
  }
  const selfTails = byRank[maxRank]
    .filter((i) => sizes.extraH[i] > 0 && sizes.selfLabelW[i] > 0)
    .map((i) => 2 + sizes.selfLabelW[i])
  const canvasW =
    rankX[maxRank] + colW[maxRank] + (selfTails.length === 0 ? 0 : Math.max(...selfTails))
  const bandEnd = Array.from({ length: maxRank + 1 }, (_, r) => rankX[r] + colW[r])
  const rankStart = rankX
  const skipRoute = skipRoutes(graph, jogs, (j) => bandEnd[j.band] + (jogTrack.get(j) ?? 0))

  let diagramH = 1
  for (let v = graph.nodes.length; v < centers.length; v++) diagramH = Math.max(diagramH, centers[v] + 1)
  byRank.forEach((row, r) => {
    const x = rankX[r]
    for (const idx of row) {
      const w = sizes.boxW[idx]
      const h = sizes.boxH[idx]
      const cy = centers[idx]
      const y = sat(cy, half(h + sizes.extraH[idx]))
      placed[idx] = { x, y, w, h, cx: x + half(w), cy: y + half(h), rank: r }
      diagramH = Math.max(diagramH, y + h + sizes.extraH[idx])
    }
  })

  const edgeLane = new Array<number>(graph.edges.length).fill(0)
  const lanes = laneSpans(graph, ranks, placed, false).filter((s) => !edgeStraight[s.edge])
  let canvasH = diagramH
  let laneBase = 0
  if (lanes.length > 0) {
    const { assigned, count } = assignTracks(lanes, true)
    for (const [idx, slot] of assigned) edgeLane[idx] = slot
    canvasH = diagramH + 1 + count
    laneBase = diagramH + 1
  }

  return {
    canvasW,
    canvasH,
    bandEnd,
    rankStart,
    edgeBus,
    laneBase,
    edgeLane,
    edgeEntryX: new Array<number>(graph.edges.length).fill(-1),
    edgeStraight,
    skipRoute,
    fwdEntryX: new Array<number>(graph.nodes.length).fill(-1),
    edgeLabelLeft: new Array<boolean>(graph.edges.length).fill(false),
  }
}

// -------------------------------------------------------------------- canvas

/** Rank, place, draw and route a graph onto a fresh canvas. */
export function layoutCanvas(graph: Graph, extras: NodeExtra[]): CanvasResult {
  const n = graph.nodes.length
  if (n === 0) return null

  // Parallel edges ride the same cells, so all labels after the first were
  // silently lost — join them onto the first instead. Done before sizing so
  // the joined label gets its room.
  const firstOf = new Map<string, number>()
  graph.edges.forEach((e, i) => {
    if (e.from === e.to) return
    const key = `${e.from}>${e.to}`
    const first = firstOf.get(key)
    if (first === undefined) {
      firstOf.set(key, i)
      return
    }
    if (e.label !== null) {
      const head = graph.edges[first].label
      graph.edges[first].label = head === null ? e.label : `${head} / ${e.label}`
      e.label = null
    }
  })

  const ranks = computeRanks(graph)
  const maxRank = Math.max(...ranks, 0)

  const byRank: number[][] = Array.from({ length: maxRank + 1 }, () => [])
  for (let idx = 0; idx < ranks.length; idx++) byRank[ranks[idx]].push(idx)
  // Lane edges exit through the rank's trailing side toward the lane strip;
  // their endpoints go last within the rank, or whatever the ordering put
  // beyond them would sit in that corridor and be cut through.
  const vertical = graph.dir === 'down' || graph.dir === 'up'
  const inLane = new Array<boolean>(graph.nodes.length).fill(false)
  for (const e of graph.edges) {
    if (vertical && isAdjacentBack(ranks, e)) continue
    if (e.from !== e.to && ranks[e.to] !== ranks[e.from] + 1) {
      inLane[e.from] = true
      inLane[e.to] = true
    }
  }
  const layered = orderRanks(byRank, graph.edges, ranks, inLane)

  const wrapped = graph.nodes.map((node) => wrapLabel(node.label, WRAP_WIDTH, MAX_LINES))
  const widest = (lines: string[]): number =>
    Math.max(1, lines.length === 0 ? 1 : Math.max(...lines.map(stringWidth)))

  const boxW = extras.map((extra, i) => {
    if (extra.kind === 'frame') {
      return Math.max(extra.sub.w + 2, stringWidth(fitLabel(graph.nodes[i].label, WRAP_WIDTH)) + 4)
    }
    if (extra.kind === 'compartments') return widest(extra.sections.flat()) + 2 * PAD + 2
    return widest(wrapped[i]) + 2 * PAD + 2
  })
  const boxH = extras.map((extra, i) => {
    if (extra.kind === 'frame') return extra.sub.h + 2
    if (extra.kind === 'compartments') {
      const filled = extra.sections.filter((s) => s.length > 0).length
      return extra.sections.reduce((s, sec) => s + sec.length, 0) + sat(filled, 1) + 2
    }
    return wrapped[i].length + 2
  })

  // A self-edge needs two rows below its box, and room beside it for a label.
  const extraH = new Array<number>(n).fill(0)
  const selfLabelW = new Array<number>(n).fill(0)
  for (const e of graph.edges) {
    if (e.from !== e.to) continue
    extraH[e.from] = 2
    const text = edgeText(e)
    if (text !== null) {
      selfLabelW[e.from] = Math.max(selfLabelW[e.from], Math.min(stringWidth(text), MAX_LABEL))
    }
  }
  for (let i = 0; i < n; i++) if (extraH[i] > 0) boxW[i] = Math.max(boxW[i], 7)

  const sizes: NodeSizes = {
    boxW,
    boxH,
    layW: boxW.map((w, i) => w + (selfLabelW[i] > 0 ? 2 * (selfLabelW[i] + 3) : 0)),
    layH: boxH.map((h, i) => h + extraH[i]),
    extraH,
    selfLabelW,
  }

  const placed: Placed[] = Array.from({ length: n }, () => ({
    x: 0,
    y: 0,
    w: 0,
    h: 0,
    cx: 0,
    cy: 0,
    rank: 0,
  }))

  const plan = vertical
    ? placeTd(ranks, maxRank, byRank, layered, sizes, graph, placed)
    : placeLr(ranks, maxRank, byRank, layered, sizes, graph, placed)

  if (plan.canvasW * plan.canvasH > MAX_CANVAS_CELLS) return null

  const canvas = new Canvas(plan.canvasW, plan.canvasH)
  for (let idx = 0; idx < n; idx++) {
    const extra = extras[idx]
    canvas.curTag = graph.nodes[idx].classes?.join(' ')
    canvas.curHref = graph.nodes[idx].href
    // `BT` mirrors the finished canvas; multi-row content draws upside down so
    // the flip restores reading order (flipHorizontal's text runs, vertically).
    const mirrored = graph.dir === 'up'
    if (extra.kind === 'frame')
      drawFrame(canvas, placed[idx], graph.nodes[idx].label, extra.sub, mirrored)
    else if (extra.kind === 'compartments')
      drawClassBox(canvas, placed[idx], extra.sections, mirrored)
    else drawBox(canvas, placed[idx], wrapped[idx], graph.nodes[idx].shape, mirrored)
  }
  canvas.curTag = undefined
  canvas.curHref = undefined

  const laneLabels: LaneLabel[] = []
  graph.edges.forEach((edge, i) => {
    canvas.curStyle =
      edge.line === 'dotted' ? STY_DOT : edge.line === 'thick' ? STY_THICK : STY_SOLID
    if (edge.from === edge.to) {
      routeSelf(canvas, placed[edge.from], edge)
      return
    }
    const from = placed[edge.from]
    const to = placed[edge.to]
    const adjacent = to.rank === from.rank + 1
    const adjacentBack = from.rank === to.rank + 1
    // A back edge crosses the band below its *target's* rank.
    const bus = plan.bandEnd[(adjacentBack ? to : from).rank] + plan.edgeBus[i]
    const lane = plan.laneBase + plan.edgeLane[i]
    if (vertical) {
      if (adjacent)
        routeForward(canvas, from, to, edge, bus, plan.fwdEntryX[edge.to], plan.edgeLabelLeft[i])
      else if (adjacentBack) routeBackAdjacent(canvas, from, to, edge, bus)
      else if (to.rank > from.rank && plan.edgeStraight[i]) {
        routeSkip(canvas, from, to, edge, plan.edgeEntryX[i], plan.skipRoute[i], plan.edgeLabelLeft[i])
      } else routeBack(canvas, from, to, edge, lane)
    } else if (adjacent) {
      routeForwardLr(canvas, from, to, edge, bus)
    } else if (to.rank > from.rank && plan.edgeStraight[i]) {
      routeSkipLr(canvas, from, to, edge, plan.skipRoute[i])
    } else {
      routeBackLr(canvas, from, to, edge, lane, laneLabels)
    }
  })
  placeLaneLabels(canvas, laneLabels)

  canvas.finalizeMask()
  return canvas
}

/** Apply the direction flip a finished canvas needs for `BT` / `RL`. */
export function orient(canvas: Canvas, graph: Graph): Canvas {
  if (graph.dir === 'up') canvas.flipVertical()
  else if (graph.dir === 'left') canvas.flipHorizontal()
  return canvas
}

/** Flowchart and state diagrams: plain boxes, no extra content. */
export function layoutFlowchart(graph: Graph): CanvasResult {
  const extras: NodeExtra[] = graph.nodes.map(() => ({ kind: 'plain' }))
  const canvas = layoutCanvas(graph, extras)
  return canvas && orient(canvas, graph)
}

/** Class and ER diagrams: boxes divided into title / attribute / method rows. */
export function layoutClass(graph: Graph): CanvasResult {
  const extras: NodeExtra[] = graph.nodes.map((node) => ({
    kind: 'compartments',
    sections: node.sections ?? [[node.label]],
  }))
  const canvas = layoutCanvas(graph, extras)
  return canvas && orient(canvas, graph)
}

// -------------------------------------------------------------------- groups

/** An endpoint inside a scope: a plain node or a (proxied) subgraph. */
interface ScopeItem {
  group: boolean
  i: number
}

/**
 * Lay out a flowchart that uses `subgraph`.
 *
 * Each subgraph becomes a framed box holding its own independently laid-out
 * canvas. An edge is drawn in the innermost scope containing both endpoints;
 * one crossing a subgraph boundary attaches to the frame instead of the node.
 */
export function layoutGrouped(graph: Graph): CanvasResult {
  // A node whose id matches a subgraph id stands in for that subgraph.
  const proxy = new Map<number, number>()
  graph.groups.forEach((g, gi) => {
    const ni = graph.index.get(g.id)
    if (ni !== undefined) proxy.set(ni, gi)
  })

  const groupChain = (g: number | null): number[] => {
    const chain: number[] = []
    let cur = g
    while (cur !== null) {
      chain.push(cur)
      cur = graph.groups[cur].parent
    }
    return chain.reverse()
  }
  const endpoint = (n: number): { item: ScopeItem; chain: number[] } => {
    const gi = proxy.get(n)
    return gi === undefined
      ? { item: { group: false, i: n }, chain: groupChain(graph.nodeGroup[n]) }
      : { item: { group: true, i: gi }, chain: groupChain(graph.groups[gi].parent) }
  }

  /** Edges bucketed by the scope that draws them; `null` is the top level. */
  const scopeEdges = new Map<number | null, [ScopeItem, ScopeItem, number][]>()
  const referenced = new Array<boolean>(graph.groups.length).fill(false)
  graph.edges.forEach((e, ei) => {
    const f = endpoint(e.from)
    const t = endpoint(e.to)
    let k = 0
    while (k < f.chain.length && k < t.chain.length && f.chain[k] === t.chain[k]) k++
    const scope = k === 0 ? null : f.chain[k - 1]
    const fItem = f.chain.length > k ? { group: true, i: f.chain[k] } : f.item
    const tItem = t.chain.length > k ? { group: true, i: t.chain[k] } : t.item
    for (const item of [fItem, tItem]) {
      if (item.group) referenced[item.i] = true
    }
    const list = scopeEdges.get(scope)
    if (list) list.push([fItem, tItem, ei])
    else scopeEdges.set(scope, [[fItem, tItem, ei]])
  })

  const directNodes = new Map<number | null, number[]>()
  graph.nodeGroup.forEach((g, ni) => {
    if (proxy.has(ni)) return
    const list = directNodes.get(g)
    if (list) list.push(ni)
    else directNodes.set(g, [ni])
  })

  // Drop empty subgraphs, but keep any that an edge attaches to. Walked by
  // the actual child relation: state `--` regions reparent earlier groups
  // under later ones, so index order says nothing about depth.
  const childGroups: number[][] = graph.groups.map(() => [])
  graph.groups.forEach((g, gi) => {
    if (g.parent !== null) childGroups[g.parent].push(gi)
  })
  const keep = new Array<boolean>(graph.groups.length).fill(false)
  const visit = (gi: number): boolean => {
    let kept = referenced[gi] || (directNodes.get(gi) ?? []).length > 0
    for (const c of childGroups[gi]) if (visit(c)) kept = true
    keep[gi] = kept
    return kept
  }
  graph.groups.forEach((g, gi) => {
    if (g.parent === null) visit(gi)
  })

  const canvas = buildScope(graph, null, scopeEdges, directNodes, keep)
  return canvas && orient(canvas, graph)
}

function buildScope(
  graph: Graph,
  scope: number | null,
  scopeEdges: Map<number | null, [ScopeItem, ScopeItem, number][]>,
  directNodes: Map<number | null, number[]>,
  keep: boolean[],
): CanvasResult {
  const items: ScopeItem[] = (directNodes.get(scope) ?? []).map((i) => ({ group: false, i }))
  const childGroups = graph.groups
    .map((_, gi) => gi)
    .filter((gi) => graph.groups[gi].parent === scope && keep[gi])
  items.push(...childGroups.map((i) => ({ group: true, i })))

  if (items.length === 0) return new Canvas(1, 1)

  const nodeAt = new Map<number, number>()
  const groupAt = new Map<number, number>()
  const nodes: Node[] = []
  const extras: NodeExtra[] = []
  for (const item of items) {
    ;(item.group ? groupAt : nodeAt).set(item.i, nodes.length)
    if (!item.group) {
      nodes.push({
        label: graph.nodes[item.i].label,
        shape: graph.nodes[item.i].shape,
        classes: graph.nodes[item.i].classes,
        href: graph.nodes[item.i].href,
      })
      extras.push({ kind: 'plain' })
    } else {
      const sub = buildScope(graph, item.i, scopeEdges, directNodes, keep)
      if (sub === null) return null
      nodes.push({ label: graph.groups[item.i].label, shape: 'rect' })
      extras.push({ kind: 'frame', sub })
    }
  }

  const edges: Edge[] = []
  for (const [f, t, ei] of scopeEdges.get(scope) ?? []) {
    const fi = (f.group ? groupAt : nodeAt).get(f.i)
    const ti = (t.group ? groupAt : nodeAt).get(t.i)
    if (fi === undefined || ti === undefined) continue
    const e = graph.edges[ei]
    edges.push({
      from: fi,
      to: ti,
      label: e.label,
      headTo: e.headTo,
      headFrom: e.headFrom,
      line: e.line,
    })
  }

  // Layout only reads nodes/edges/dir, so a bare Graph carrying those is enough.
  const synth = new Graph(graph.dir)
  synth.nodes = nodes
  synth.edges = edges
  return layoutCanvas(synth, extras)
}

// ------------------------------------------------------------------- drawing

export function drawBox(
  canvas: Canvas,
  p: Placed,
  lines: string[],
  shape: Shape,
  mirrored = false,
): void {
  const { x, y, w, h } = p
  const right = x + w - 1
  const bottom = y + h - 1

  // A diamond is a double-line box — the terminal's nod to `A{...}`.
  const [tl, tr, bl, br] =
    shape === 'diamond'
      ? ['╔', '╗', '╚', '╝']
      : shape === 'round'
        ? ['╭', '╮', '╰', '╯']
        : ['┌', '┐', '└', '┘']
  canvas.set(x, y, tl, 'border')
  canvas.set(right, y, tr, 'border')
  canvas.set(x, bottom, bl, 'border')
  canvas.set(right, bottom, br, 'border')

  if (shape === 'diamond') {
    // Double lines have no direction bits; edges tee into them through the
    // mixed junctions (`╤` `╧` `╟` `╢`) that `finalizeMask` resolves.
    for (let cx = x + 1; cx < right; cx++) {
      canvas.set(cx, y, '═', 'border')
      canvas.set(cx, bottom, '═', 'border')
    }
    for (let cy = y + 1; cy < bottom; cy++) {
      canvas.set(x, cy, '║', 'border')
      canvas.set(right, cy, '║', 'border')
    }
  } else {
    // The perimeter is drawn as bits so edges can tee into it, but it is the
    // box outline, so it claims `border` rather than `edge`.
    for (let cx = x + 1; cx < right; cx++) {
      canvas.addBits(cx, y, L | R, 'border')
      canvas.addBits(cx, bottom, L | R, 'border')
    }
    for (let cy = y + 1; cy < bottom; cy++) {
      canvas.addBits(x, cy, U | D, 'border')
      canvas.addBits(right, cy, U | D, 'border')
    }
  }

  for (let cy = y; cy <= bottom; cy++) {
    for (let cx = x; cx <= right; cx++) {
      const i = canvas.idx(cx, cy)
      canvas.occupied[i] = 1
      if (canvas.curTag !== undefined) canvas.tag[i] = canvas.curTag
      if (canvas.curHref !== undefined) canvas.href[i] = canvas.curHref
    }
  }

  const inner = Math.max(1, sat(w, 2 * PAD + 2))
  const ordered = mirrored ? [...lines].reverse() : lines
  ordered.forEach((line, li) => {
    const text = fitLabel(line, inner)
    const textX = x + 1 + PAD + half(sat(inner, stringWidth(text)))
    drawText(canvas, text, textX, y + 1 + li, 'text')
  })
}

/** A class or ER box: sections separated by horizontal rules, title centred. */
function drawClassBox(canvas: Canvas, p: Placed, sections: string[][], mirrored = false): void {
  drawBox(canvas, p, [], 'rect')
  const inner = Math.max(1, sat(p.w, 2 * PAD + 2))
  const rows: ({ sep: true } | { sep?: undefined; text: string; center: boolean })[] = []
  sections.forEach((section, si) => {
    if (section.length === 0) return
    if (rows.length > 0) rows.push({ sep: true })
    for (const line of section) rows.push({ text: fitLabel(line, inner), center: si === 0 })
  })
  if (mirrored) rows.reverse()
  rows.forEach((r, ri) => {
    const row = p.y + 1 + ri
    if (r.sep) {
      canvas.set(p.x, row, '├', 'border')
      for (let x = p.x + 1; x < p.x + p.w - 1; x++) canvas.set(x, row, '─', 'border')
      canvas.set(p.x + p.w - 1, row, '┤', 'border')
    } else {
      const tx = r.center ? p.x + 1 + PAD + half(sat(inner, stringWidth(r.text))) : p.x + 1 + PAD
      drawTextOverEdges(canvas, r.text, tx, row, 'text')
    }
  })
}

/** A subgraph frame: a titled box with a finished sub-canvas centred inside. */
function drawFrame(canvas: Canvas, p: Placed, title: string, sub: Canvas, mirrored = false): void {
  drawBox(canvas, p, [], 'rect')
  // An unlabelled frame (a state `--` region) keeps its border unbroken.
  if (title !== '') {
    const t = fitLabel(title, sat(p.w, 4))
    // Mirrored: the bottom border becomes the top after the flip.
    drawTextOverEdges(canvas, ` ${t} `, p.x + 1, mirrored ? p.y + p.h - 1 : p.y, 'text')
  }
  canvas.blit(sub, p.x + 1 + half(p.w - 2 - sub.w), p.y + 1 + half(p.h - 2 - sub.h))
}

// ------------------------------------------------------------------- routing

function headGlyph(head: Head, arrow: string): string {
  switch (head) {
    case 'circle':
      return 'o'
    case 'cross':
      return '×'
    case 'diamondFill':
      return '◆'
    case 'diamondOpen':
      return '◇'
    case 'triangle':
      return { '▼': '▽', '▲': '△', '◄': '◁', '▶': '▷' }[arrow] ?? arrow
    default:
      return arrow
  }
}

/** Adjacent ranks, top-down: drop, jog along the bus row, drop into the head.
 * `entryX` overrides the centre entry column when the target's top is shared
 * with skip entries; `labelLeft` renders the label left of the arrowhead. */
function routeForward(
  canvas: Canvas,
  from: Placed,
  to: Placed,
  edge: Edge,
  bus: number,
  entryX = -1,
  labelLeft = false,
): void {
  const tx = entryX === -1 ? to.cx : entryX
  // A jog of one column reads as a kink; snap straight instead.
  const bx = Math.abs(from.cx - tx) <= 1 ? tx : from.cx
  const by = from.y + from.h - 1
  const headRow = to.y - 1

  canvas.junction(bx, by, D)
  canvas.segV(bx, by, bus)
  if (bx === tx) {
    canvas.segV(bx, bus, headRow)
  } else {
    canvas.segH(bus, bx, tx)
    canvas.segV(tx, bus, headRow)
  }

  if (edge.headTo === 'none') canvas.addBits(tx, headRow, U)
  else canvas.set(tx, headRow, headGlyph(edge.headTo, '▼'), 'edge')
  if (edge.headFrom !== 'none') canvas.set(bx, by, headGlyph(edge.headFrom, '▲'), 'edge')

  if (edge.cardFrom === undefined && edge.cardTo === undefined) {
    if (edge.label !== null) {
      const start = labelLeft ? sat(tx, Math.min(stringWidth(edge.label), MAX_LABEL)) : tx + 1
      placeLabel(canvas, edge.label, headRow, start)
    }
    return
  }
  // Cardinalities sit at their own ends; the verb takes the row above the
  // head, falling back beside the target card when the gap has no spare row.
  const srcRow = by + 1
  if (edge.cardFrom !== undefined) placeLabel(canvas, edge.cardFrom, srcRow, bx + 1)
  if (edge.cardTo !== undefined) placeLabel(canvas, edge.cardTo, headRow, tx + 1)
  if (edge.label !== null) {
    const midRow = headRow - 1
    if (midRow > srcRow) {
      const lineX = midRow > bus ? tx : bx
      placeLabel(canvas, edge.label, midRow, lineX + 1)
    } else {
      placeLabel(
        canvas,
        edge.label,
        headRow,
        tx + 1 + (edge.cardTo === undefined ? 0 : stringWidth(edge.cardTo) + 1),
      )
    }
  }
}

/**
 * Adjacent ranks, top-down, against the flow: up out of the source's top,
 * jog along the band, arrow into the target's bottom. The short local return
 * mermaid draws — not a trip around the diagram.
 */
function routeBackAdjacent(
  canvas: Canvas,
  from: Placed,
  to: Placed,
  edge: Edge,
  bus: number,
): void {
  // Attach right of centre so the return does not merge with the forward
  // exits and arrivals that own the centre column.
  const tx = Math.min(to.x + to.w - 2, to.cx + 2)
  const bx0 = Math.min(from.x + from.w - 2, from.cx + 2)
  const bx = Math.abs(bx0 - tx) <= 1 ? tx : bx0
  const fy = from.y
  const headRow = to.y + to.h

  canvas.junction(bx, fy, U)
  if (bx === tx) {
    canvas.segV(bx, headRow, fy)
  } else {
    canvas.segV(bx, bus, fy)
    canvas.segH(bus, bx, tx)
    canvas.segV(tx, headRow, bus)
  }

  if (edge.headTo === 'none') canvas.addBits(tx, headRow, D)
  else canvas.set(tx, headRow, headGlyph(edge.headTo, '▲'), 'edge')
  if (edge.headFrom !== 'none') canvas.set(bx, fy, headGlyph(edge.headFrom, '▼'), 'edge')

  const text = edgeText(edge)
  if (text !== null) placeLabel(canvas, text, headRow, tx + 1)
}

/** A self-edge: a stub loop hanging below the box. */
function routeSelf(canvas: Canvas, p: Placed, edge: Edge): void {
  const bottom = p.y + p.h - 1
  const exitX = p.cx + 1
  const retX = p.x + p.w - 2
  if (retX <= exitX || bottom + 2 >= canvas.h) return

  const [v, h, bl, br] =
    edge.line === 'dotted'
      ? ['╎', '╌', '╰', '╯']
      : edge.line === 'thick'
        ? ['┃', '━', '┗', '┛']
        : ['│', '─', '╰', '╯']

  canvas.junction(exitX, bottom, D)
  canvas.set(exitX, bottom + 1, v, 'edge')
  canvas.set(exitX, bottom + 2, bl, 'edge')
  for (let x = exitX + 1; x < retX; x++) canvas.set(x, bottom + 2, h, 'edge')
  canvas.set(retX, bottom + 2, br, 'edge')
  canvas.set(retX, bottom + 1, headGlyph(edge.headTo, '▲'), 'edge')
  const selfText = edgeText(edge)
  if (selfText !== null) placeLabel(canvas, selfText, bottom + 1, p.x + p.w + 1)
}

/**
 * Forward skip edge, top-down: out the source's *bottom*, then down the
 * column its virtual chain reserved, jogging along a bus row wherever the
 * chain steps sideways (the first jog shares the source's fan row — one `┴`
 * origin split; the last lands on the entry column) into the target's *top*.
 */
function routeSkip(
  canvas: Canvas,
  from: Placed,
  to: Placed,
  edge: Edge,
  entryX: number,
  route: { bus: number; at: number }[],
  labelLeft: boolean,
): void {
  const bx = from.cx
  const bottom = from.y + from.h - 1
  const headRow = to.y - 1

  canvas.junction(bx, bottom, D)
  let x = bx
  let y = bottom
  for (const { bus, at } of route) {
    canvas.segV(x, y, bus)
    canvas.segH(bus, x, at)
    x = at
    y = bus
  }
  canvas.segV(x, y, headRow)

  if (edge.headTo === 'none') canvas.addBits(entryX, headRow, D)
  else canvas.set(entryX, headRow, headGlyph(edge.headTo, '▼'), 'edge')
  if (edge.headFrom !== 'none') canvas.set(bx, bottom, headGlyph(edge.headFrom, '▲'), 'edge')

  const text = edgeText(edge)
  if (text !== null) {
    const start = labelLeft ? sat(entryX, Math.min(stringWidth(text), MAX_LABEL)) : entryX + 1
    placeLabel(canvas, text, headRow, start)
  }
}

/**
 * Multi-rank back edge, top-down: out the right side, up a lane, back in
 * through the target's right side — side entry keeps returns recognisable
 * at a glance.
 */
function routeBack(canvas: Canvas, from: Placed, to: Placed, edge: Edge, laneX: number): void {
  const sx = from.x + from.w - 1
  const sy = from.cy
  const backText = edgeText(edge)

  canvas.junction(sx, sy, R)
  canvas.segH(sy, sx, laneX)
  if (edge.headFrom !== 'none') canvas.set(sx, sy, headGlyph(edge.headFrom, '◄'), 'edge')

  const tx = to.x + to.w - 1
  const tyc = to.cy
  canvas.segV(laneX, sy, tyc)
  canvas.segH(tyc, tx + 1, laneX)
  if (edge.headTo === 'none') canvas.addBits(tx + 1, tyc, R)
  else canvas.set(tx + 1, tyc, headGlyph(edge.headTo, '◄'), 'edge')
  if (backText !== null) {
    placeLabel(canvas, backText, sat(tyc, 1), sat(laneX, stringWidth(backText) + 1))
  }
}

/** Adjacent ranks, left-to-right: out the right side, jog on the bus column. */
function routeForwardLr(canvas: Canvas, from: Placed, to: Placed, edge: Edge, bus: number): void {
  const rx = from.x + from.w - 1
  const ry = from.cy
  const ly = to.cy
  const headCol = to.x - 1

  canvas.junction(rx, ry, R)
  canvas.segH(ry, rx, bus)
  if (ry === ly) {
    canvas.segH(ry, bus, headCol)
  } else {
    canvas.segV(bus, ry, ly)
    canvas.segH(ly, bus, headCol)
  }

  if (edge.headTo === 'none') canvas.addBits(headCol, ly, R)
  else canvas.set(headCol, ly, headGlyph(edge.headTo, '▶'), 'edge')
  if (edge.headFrom !== 'none') canvas.set(rx, ry, headGlyph(edge.headFrom, '◄'), 'edge')

  // The verb keeps its usual spot above the line; cardinalities hug their
  // own ends on the rows above the departure and arrival cells.
  if (edge.label !== null) placeLabel(canvas, edge.label, sat(ly, 1), bus + 1)
  if (edge.cardFrom !== undefined) placeLabel(canvas, edge.cardFrom, sat(ry, 1), rx + 1)
  if (edge.cardTo !== undefined) {
    placeLabel(canvas, edge.cardTo, sat(ly, 1), sat(headCol, stringWidth(edge.cardTo)))
  }
}

/**
 * Forward skip, left-to-right: out the source's right side, along the row
 * its virtual chain reserved, jogging on a bus column wherever the chain
 * steps (the first jog shares the source's fan column), into the target's
 * left side on its centre row.
 */
function routeSkipLr(
  canvas: Canvas,
  from: Placed,
  to: Placed,
  edge: Edge,
  route: { bus: number; at: number }[],
): void {
  const rx = from.x + from.w - 1
  const ry = from.cy
  const ty = to.cy
  const headCol = to.x - 1

  canvas.junction(rx, ry, R)
  let x = rx
  let y = ry
  for (const { bus, at } of route) {
    canvas.segH(y, x, bus)
    canvas.segV(bus, y, at)
    x = bus
    y = at
  }
  canvas.segH(ty, x, headCol)

  if (edge.headTo === 'none') canvas.addBits(headCol, ty, R)
  else canvas.set(headCol, ty, headGlyph(edge.headTo, '▶'), 'edge')
  if (edge.headFrom !== 'none') canvas.set(rx, ry, headGlyph(edge.headFrom, '◄'), 'edge')

  // Label after the first jog, where forward labels sit — the gap before
  // the target belongs to the arrivals that end there.
  const text = edgeText(edge)
  if (text !== null) placeLabel(canvas, text, sat(route[0]?.at ?? ty, 1), (route[0]?.bus ?? rx) + 1)
}

/** A lane label waiting for every route to land before claiming its spot. */
interface LaneLabel {
  text: string
  y: number
  lo: number
  hi: number
}

/** Skip or back edge, left-to-right: down out the bottom, along a lane, back up. */
function routeBackLr(
  canvas: Canvas,
  from: Placed,
  to: Placed,
  edge: Edge,
  laneY: number,
  laneLabels: LaneLabel[],
): void {
  const sx = from.cx
  const sy = from.y + from.h - 1
  const tx = to.cx
  const ty = to.y + to.h - 1

  canvas.junction(sx, sy, D)
  canvas.segV(sx, sy, laneY)
  canvas.segH(laneY, sx, tx)
  canvas.segV(tx, laneY, ty + 1)

  if (edge.headTo === 'none') canvas.addBits(tx, ty + 1, D)
  else canvas.set(tx, ty + 1, headGlyph(edge.headTo, '▲'), 'edge')
  if (edge.headFrom !== 'none') canvas.set(sx, sy, headGlyph(edge.headFrom, '▲'), 'edge')

  // The label interrupts its own lane row — the row above belongs to the
  // neighbouring lane once several stack. Deferred until all edges landed,
  // so it can dodge the verticals that cross this row.
  const backText = edgeText(edge)
  if (backText !== null) {
    laneLabels.push({
      text: ` ${fitLabel(backText, MAX_LABEL)} `,
      y: laneY,
      lo: Math.min(sx, tx),
      hi: Math.max(sx, tx),
    })
  }
}

/**
 * Write each lane label onto its own row, centred on the run but slid to
 * the nearest stretch free of crossing verticals, arrowheads and earlier
 * labels — clearing a crossing line under a label would sever it.
 */
function placeLaneLabels(canvas: Canvas, labels: LaneLabel[]): void {
  for (const { text, y, lo, hi } of labels) {
    const tw = stringWidth(text)
    const lastStart = hi - 1 - tw
    if (lastStart < lo + 1 || y >= canvas.h) continue
    const clear = (start: number): boolean => {
      for (let x = start; x < start + tw; x++) {
        const i = canvas.idx(x, y)
        if (canvas.occupied[i] === 1) return false
        if ((canvas.mask[i] & (U | D)) !== 0) return false
        if (canvas.ch[i] !== ' ') return false
      }
      return true
    }
    const mid = Math.min(Math.max(half(lo + hi) - half(tw), lo + 1), lastStart)
    let at = mid
    for (let d = 0; ; d++) {
      const left = mid - d
      const right = mid + d
      if (left < lo + 1 && right > lastStart) break
      if (left >= lo + 1 && clear(left)) {
        at = left
        break
      }
      if (right <= lastStart && clear(right)) {
        at = right
        break
      }
    }
    drawTextOverEdges(canvas, text, at, y, 'edgeLabel')
  }
}

/** Write an edge label, stopping at the first cell already occupied. */
function placeLabel(canvas: Canvas, label: string, row: number, startX: number): void {
  if (row >= canvas.h) return
  const text = fitLabel(label, MAX_LABEL)
  let x = startX
  for (const [c, cw] of measured(text)) {
    if (cw === 0) continue
    if (x + cw > canvas.w) break
    let blocked = false
    for (let k = 0; k < cw; k++) {
      const i = canvas.idx(x + k, row)
      if (canvas.ch[i] !== ' ' || canvas.mask[i] !== 0 || canvas.occupied[i]) blocked = true
    }
    if (blocked) break
    canvas.set(x, row, c, 'edgeLabel')
    for (let k = 1; k < cw; k++) canvas.set(x + k, row, CONT, 'edgeLabel')
    x += cw
  }
}
