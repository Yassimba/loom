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
  /** Skip edges: the column entering the target's top; back edges: the
   * column entering its bottom. -1 otherwise. */
  edgeEntryX: number[]
  /** Back edges, top-down: the column leaving the source's top. */
  edgeExitX: number[]
  /** Skip edges routed through the interior along their virtual chain. */
  edgeStraight: boolean[]
  /**
   * Interior skip route: each jog's bus coordinate and the cross-axis
   * coordinate the edge continues along after it, in flow order.
   */
  skipRoute: { bus: number; at: number }[][]
  /** Per node: the forward cluster's entry column, or -1 for the centre. */
  /** Per edge: render the label left of the arrowhead instead of right. */
  edgeLabelLeft: boolean[]
  /** Where a chain-routed edge's label sits beside its long vertical, if it moved off the head row. */
  edgeLabelAt: ({ row: number; x: number } | null)[]
}

// ------------------------------------------------------------------ ranking

/**
 * Rank assignment along the flow axis.
 *
 * Cycles are broken by a DFS colouring pass in declaration order, so the
 * edge treated as the return is the one the author wrote against the flow
 * (`A --> B --> C --> A` returns on `C --> A`); greedy feedback-set
 * heuristics reverse fewer edges on random graphs but ignore that order.
 * Reversed edges take part in ranking in their reversed direction, so a
 * return always climbs at least one rank. Longest-path layering puts each
 * node as early as its predecessors allow, then Nikolov's node promotion
 * (mirrored: nodes move later) shortens edges while that removes more
 * virtual chain nodes than it adds.
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
  const tree: number[][] = Array.from({ length: n }, () => [])
  const postorder: number[] = []
  // Roots first so ranks grow from natural entry points, then any leftovers.
  const roots = [...Array(n).keys()].filter((i) => indeg[i] === 0)
  for (const start of [...roots, ...Array(n).keys()]) {
    if (color[start] === 0) dfsDag(start, children, color, tree, postorder)
  }
  const forward = new Set<string>()
  tree.forEach((vs, u) => {
    for (const v of vs) forward.add(`${u}>${v}`)
  })

  const succ: number[][] = Array.from({ length: n }, () => [])
  const pred: number[][] = Array.from({ length: n }, () => [])
  for (const e of graph.edges) {
    if (e.from === e.to) continue
    const [a, b] = forward.has(`${e.from}>${e.to}`) ? [e.from, e.to] : [e.to, e.from]
    succ[a].push(b)
    pred[b].push(a)
  }
  const order = [...postorder].reverse()

  const rank = new Array<number>(n).fill(0)
  for (const u of order) for (const v of succ[u]) rank[v] = Math.max(rank[v], rank[u] + 1)

  // Demote a node (and whatever it would collide with) one rank later;
  // worth keeping when the virtual nodes saved on its incoming edges
  // outnumber those added on its outgoing ones.
  const demote = (v: number): number => {
    let saved = 0
    for (const w of succ[v]) if (rank[w] === rank[v] + 1) saved += demote(w)
    rank[v]++
    return saved + succ[v].length - pred[v].length
  }
  for (let round = 0; round < 8; round++) {
    let improved = false
    for (let v = 0; v < n; v++) {
      if (succ[v].length === 0) continue
      const before = [...rank]
      if (demote(v) > 0) improved = true
      else rank.splice(0, n, ...before)
    }
    if (!improved) break
  }
  const min = Math.min(...rank, 0)
  return rank.map((r) => r - min)
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
  /** Virtual nodes on more than one chain (a concentrated trunk). */
  shared: Set<number>
}

/**
 * Split each edge into unit-rank segments: forward adjacent edges and the
 * ones `interior` accepts take part, the rest run around the outside and
 * are left out. A chain is listed in the edge's own direction, so a back
 * edge's runs up the ranks.
 *
 * Edges leaving one node share virtual nodes for as long as they all
 * continue (dot's `concentrate`): the fan runs as one trunk that splits
 * where the first target arrives, one column per rank instead of one per
 * edge. Edges arriving at one node share the same way on their last
 * ranks. A node is never shared both ways, which would join two edges
 * with neither end in common and read as a third. Naive normalisation is
 * bounded by MAX_EDGES × MAX_NODES virtual nodes, small enough here.
 */
function normalize(
  byRank: number[][],
  edges: Edge[],
  ranks: number[],
  interior: (e: Edge) => boolean,
): Layered {
  const n = ranks.length
  const layers = byRank.map((row) => [...row])
  const up: number[][] = Array.from({ length: n }, () => [])
  const down: number[][] = Array.from({ length: n }, () => [])
  const link = (a: number, b: number, upward: boolean): void => {
    const [hi, lo] = upward ? [b, a] : [a, b]
    if (down[hi].includes(lo)) return
    down[hi].push(lo)
    up[lo].push(hi)
  }
  const chains: number[][] = edges.map(() => [])
  const owners = new Map<number, number>()
  const shared = new Set<number>()
  const trunks = new Map<string, number>()
  const takes = (e: Edge): boolean =>
    e.from !== e.to && (ranks[e.to] === ranks[e.from] + 1 || interior(e))
  // How far from each end a group of edges keeps company: up to the
  // second farthest endpoint among edges sharing that end, since sharing
  // needs two.
  const reach = (key: 'from' | 'to'): Map<number, number> => {
    const other = key === 'from' ? 'to' : 'from'
    const ends = new Map<number, number[]>()
    for (const e of edges) {
      if (!takes(e)) continue
      const list = ends.get(e[key]) ?? []
      list.push(ranks[e[other]])
      ends.set(e[key], list)
    }
    const out = new Map<number, number>()
    for (const [node, rs] of ends) {
      const d = rs.map((r) => Math.abs(r - ranks[node])).sort((a, b) => a - b)
      if (d.length > 1) out.set(node, d[d.length - 2])
    }
    return out
  }
  const fromReach = reach('from')
  const toReach = reach('to')
  edges.forEach((e, i) => {
    if (!takes(e)) return
    const upward = ranks[e.to] < ranks[e.from]
    const step = upward ? -1 : 1
    const span = Math.abs(ranks[e.to] - ranks[e.from])
    const headEnd = Math.min(fromReach.get(e.from) ?? 0, span) - 1
    const tailStart = span - Math.min(toReach.get(e.to) ?? 0, span) + 1
    let prev = e.from
    for (let k = 1; k < span; k++) {
      const r = ranks[e.from] + step * k
      const key = k <= headEnd ? `f${e.from}@${r}` : k >= tailStart && k > headEnd ? `t${e.to}@${r}` : null
      let v = key === null ? undefined : trunks.get(key)
      if (v === undefined) {
        v = up.length
        up.push([])
        down.push([])
        layers[r].push(v)
        if (key !== null) trunks.set(key, v)
        owners.set(v, i)
      } else shared.add(v)
      chains[i].push(v)
      link(prev, v, upward)
      prev = v
    }
    link(prev, e.to, upward)
  })
  return { layers, up, down, chains, shared }
}

/**
 * Reorder nodes within each rank to minimise edge crossings.
 *
 * Edges `interior` accepts (the ones later routed through the diagram
 * rather than around it) are normalised into virtual-node chains first, so every boundary crossing is
 * visible to the count and a long edge is ordered as one coherent chain;
 * the rest run around the outside and are ignored here. Alternate down/up barycenter sweeps are each followed
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
  interior: (e: Edge) => boolean,
  trailing: boolean[] = [],
): Layered {
  const n = ranks.length
  const isTrailing = (v: number): boolean => trailing[v] ?? false
  const partition = (row: number[]): void => {
    row.sort((a, b) => Number(isTrailing(a)) - Number(isTrailing(b)))
  }
  for (const row of byRank) partition(row)
  const layered = normalize(byRank, edges, ranks, interior)
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
  const sweep = (): void => {
    let stale = 0
    let current = total()
    for (let it = 0; current > 0 && stale < 2 && it < 24; it++) {
      const downward = it % 2 === 0
      const rows = downward ? layers.slice(1) : layers.slice(0, -1).reverse()
      const neigh = downward ? up : down
      for (const row of rows) {
        sortByMedian(row, neigh, pos)
        partition(row)
        reindex(row)
      }
      transpose(layers, up, down, pos, isTrailing)
      const crossings = total()
      if (crossings < current) {
        current = crossings
        stale = 0
      } else stale++
      if (crossings < bestCrossings) {
        bestCrossings = crossings
        best = layers.map((row) => [...row])
      }
    }
  }
  // The sweeps settle into a local minimum shaped by the starting order:
  // declaration order first, then a few seeded shuffles, best kept.
  let seed = 0x9e3779b9
  const random = (): number => {
    seed = (Math.imul(seed, 1103515245) + 12345) >>> 0
    return seed / 0x100000000
  }
  for (let restart = 0; restart < 4 && bestCrossings > 0; restart++) {
    if (restart > 0) {
      for (const row of layers) {
        for (let i = row.length - 1; i > 0; i--) {
          const j = Math.floor(random() * (i + 1))
          ;[row[i], row[j]] = [row[j], row[i]]
        }
        partition(row)
        reindex(row)
      }
    }
    sweep()
  }

  for (let i = 0; i < byRank.length; i++) {
    byRank[i].splice(0, byRank[i].length, ...best[i].filter((v) => v < n))
  }
  return { ...layered, layers: best }
}

/**
 * Sort a rank by each node's weighted median neighbour position (Gansner
 * et al.): the median for an odd count, the mean of the two middle ones
 * for two, otherwise the two middle ones weighted toward the side whose
 * neighbours spread less. A node without neighbours keeps its place.
 */
function sortByMedian(row: number[], neigh: number[][], pos: number[]): void {
  const key = (v: number): number => {
    const p = neigh[v].map((u) => pos[u]).sort((a, b) => a - b)
    const m = p.length >> 1
    if (p.length === 0) return pos[v]
    if (p.length % 2 === 1) return p[m]
    if (p.length === 2) return (p[0] + p[1]) / 2
    const left = p[m - 1] - p[0]
    const right = p[p.length - 1] - p[m]
    return left + right === 0 ? (p[m - 1] + p[m]) / 2 : (p[m - 1] * right + p[m] * left) / (left + right)
  }
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
  offset: (v: number) => number = () => 0,
  padLeft: (node: number) => number = () => 0,
): number[] {
  const n = size.length
  const all = [...size]
  while (all.length < layered.up.length) all.push(1)
  // `pad(v)` reserves cells right of `v` for a label: a real node's arrival
  // labels when a chain follows it, or a chain node's own edge label;
  // `padLeft(v)` the same on its left.
  const sepOf = (left: number, right: number): number =>
    left < n && right < n ? sep : 1 + (left >= n || right >= n ? pad(left) : 0) + padLeft(right)
  return brandesKoepf(layered, all, sepOf, n, offset)
}

// ------------------------------------------------------------------- tracks

/**
 * A span competing for a track: the covered coordinate range, the arms
 * that reach it from either side, and its edge. In a band between ranks,
 * `up` arms come from the earlier rank and `down` arms lead on to the
 * later one (in a lane strip both arms are `up`).
 */
interface TrackSpan {
  start: number
  end: number
  from: number
  to: number
  edge: number
  up: number[]
  down: number[]
  /** A labelled lane refuses endpoint sharing: the label would appear to
   * cover every edge merged onto the row. Bus spans never set this — their
   * labels sit at the separate arrival ends, so fan merging stays safe. */
  labeled?: boolean
}

/** Spans merged onto one track because they share an endpoint. */
interface Hyper {
  members: TrackSpan[]
  start: number
  end: number
  up: number[]
  down: number[]
}

/**
 * Order spans onto parallel tracks, nearest the earlier rank first, so
 * that arms cross as few other spans' runs as possible (Sander's segment
 * ordering, as in ELK's orthogonal router): spans that share an endpoint
 * merge into one run — edges fanning out of one node draw one `┴` origin
 * rather than a stack of them — then every two runs that overlap are
 * compared both ways, the cheaper order becomes a dependency, cycles are
 * broken greedily, and a run's track is its longest dependency path. Runs
 * two cells apart share a track.
 */
export function assignTracks(spans: TrackSpan[]): { assigned: [number, number][]; count: number } {
  const hypers = mergeShared(spans)
  const n = hypers.length
  const overlaps = (a: Hyper, b: Hyper): boolean => a.start <= b.end + 1 && b.start <= a.end + 1
  /** Crossings when `a` runs on the track nearer the earlier rank than `b`. */
  const crossings = (a: Hyper, b: Hyper): number =>
    a.down.filter((x) => b.start < x && x < b.end).length +
    b.up.filter((x) => a.start < x && x < a.end).length
  const weight: number[][] = Array.from({ length: n }, () => new Array<number>(n).fill(-1))
  for (let i = 0; i < n; i++) {
    for (let j = i + 1; j < n; j++) {
      if (!overlaps(hypers[i], hypers[j])) continue
      const ij = crossings(hypers[i], hypers[j])
      const ji = crossings(hypers[j], hypers[i])
      // Equal: keep the earlier-starting run nearer, the packing order.
      if (ij < ji || (ij === ji && hypers[i].start <= hypers[j].start)) weight[i][j] = ji - ij
      else weight[j][i] = ij - ji
    }
  }
  const order = greedyAcyclic(weight)
  const track = new Array<number>(n).fill(0)
  for (const v of order) {
    for (let u = 0; u < n; u++) {
      if (weight[u][v] >= 0 && track[u] + 1 > track[v]) track[v] = track[u] + 1
    }
  }
  const assigned: [number, number][] = []
  hypers.forEach((h, i) => {
    for (const m of h.members) assigned.push([m.edge, track[i]])
  })
  return { assigned, count: n === 0 ? 0 : Math.max(...track) + 1 }
}

/**
 * Pack lane spans into as few tracks as possible, shortest first: a span
 * contained in another takes the inner track, so exits and entries at rows
 * the inner lane never reaches cross nothing. Lanes trade crossings for
 * height, where `assignTracks`' dependency chains would cost a track each.
 */
export function packTracks(spans: TrackSpan[]): { assigned: [number, number][]; count: number } {
  const sorted = [...spans].sort(
    (a, b) =>
      a.end - a.start - (b.end - b.start) ||
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

function mergeShared(spans: TrackSpan[]): Hyper[] {
  const sorted = [...spans].sort(
    (a, b) => a.start - b.start || a.end - b.end || a.from - b.from || a.to - b.to || a.edge - b.edge,
  )
  const hypers: Hyper[] = []
  for (const span of sorted) {
    const host =
      span.labeled === true
        ? undefined
        : hypers.find((h) =>
            h.members.some(
              (m) =>
                m.labeled !== true &&
                ((m.from === span.from && m.up[0] === span.up[0]) ||
                  (m.to === span.to && m.down[0] === span.down[0])),
            ),
          )
    if (host === undefined) {
      hypers.push({ members: [span], start: span.start, end: span.end, up: [...span.up], down: [...span.down] })
      continue
    }
    host.members.push(span)
    host.start = Math.min(host.start, span.start)
    host.end = Math.max(host.end, span.end)
    host.up.push(...span.up)
    host.down.push(...span.down)
  }
  return hypers
}

/**
 * Eades–Lin–Smyth greedy cycle removal on a weighted dependency matrix:
 * returns a vertex order; dependencies pointing backwards in it are
 * dropped (set to -1). Sinks go last, sources first, else the vertex with
 * the largest outgoing-minus-incoming weight goes first.
 */
function greedyAcyclic(weight: number[][]): number[] {
  const n = weight.length
  const alive = new Array<boolean>(n).fill(true)
  const head: number[] = []
  const tail: number[] = []
  const sum = (v: number, incoming: boolean): number => {
    let total = 0
    for (let u = 0; u < n; u++) {
      const w = incoming ? weight[u][v] : weight[v][u]
      if (alive[u] && w >= 0) total += w + 1
    }
    return total
  }
  let left = n
  while (left > 0) {
    let progressed = false
    for (let v = 0; v < n; v++) {
      if (!alive[v]) continue
      if (sum(v, false) === 0) {
        tail.push(v)
        alive[v] = false
        left--
        progressed = true
      } else if (sum(v, true) === 0) {
        head.push(v)
        alive[v] = false
        left--
        progressed = true
      }
    }
    if (progressed || left === 0) continue
    let best = -1
    let bestScore = Number.NEGATIVE_INFINITY
    for (let v = 0; v < n; v++) {
      if (!alive[v]) continue
      const score = sum(v, false) - sum(v, true)
      if (score > bestScore) {
        bestScore = score
        best = v
      }
    }
    head.push(best)
    alive[best] = false
    left--
  }
  const order = [...head, ...tail.reverse()]
  const pos = new Array<number>(n).fill(0)
  order.forEach((v, i) => (pos[v] = i))
  for (let u = 0; u < n; u++) for (let v = 0; v < n; v++) if (weight[u][v] >= 0 && pos[u] > pos[v]) weight[u][v] = -1
  return order
}

/** Forward edges crossing the band between rank `r` and `r + 1` that must
 * jog sideways, so need a bus row. */
function busSpans(
  graph: Graph,
  ranks: number[],
  centers: number[],
  r: number,
  exact: boolean,
  entry: (edge: number) => number = (i) => centers[graph.edges[i].to],
): TrackSpan[] {
  const out: TrackSpan[] = []
  graph.edges.forEach((e, i) => {
    const jogs = exact
      ? centers[e.from] !== centers[e.to]
      : Math.abs(centers[e.from] - centers[e.to]) > 1
    if (e.from !== e.to && ranks[e.to] === ranks[e.from] + 1 && ranks[e.from] === r && jogs) {
      const arrive = exact ? centers[e.to] : entry(i)
      out.push({
        start: Math.min(centers[e.from], arrive),
        end: Math.max(centers[e.from], arrive),
        from: e.from,
        to: e.to,
        edge: i,
        up: [centers[e.from]],
        down: [arrive],
      })
    }
  })
  return out
}

/** Left-to-right edges skipping a rank or running backwards that go around
 * in a lane below the diagram. */
function laneSpans(graph: Graph, ranks: number[], placed: Placed[]): TrackSpan[] {
  const out: TrackSpan[] = []
  graph.edges.forEach((e, i) => {
    if (e.from === e.to || ranks[e.to] === ranks[e.from] + 1) return
    const pf = placed[e.from]
    const pt = placed[e.to]
    const a = Math.min(pf.cx, pt.cx)
    const b = Math.max(pf.cx, pt.cx)
    out.push({
      start: a,
      end: b,
      from: e.from,
      to: e.to,
      edge: i,
      up: [pf.cx, pt.cx],
      down: [],
      labeled: edgeText(e) !== null,
    })
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
 * The jogs an interior edge makes following its virtual chain: exit
 * coordinate to the first chain coordinate, between chain nodes where they
 * differ, and from the last one to the entry coordinate. Edges `exit`
 * returns `null` for take no part (they stay on a lane). A back edge walks
 * its bands upward.
 */
function chainJogs(
  graph: Graph,
  ranks: number[],
  layered: Layered,
  centers: number[],
  ends: (e: Edge, i: number) => { exit: number; entry: number } | null,
): ChainJog[] {
  const jogs: ChainJog[] = []
  graph.edges.forEach((e, i) => {
    const at = ends(e, i)
    if (at === null) return
    const chain = layered.chains[i]
    const stops = [at.exit, ...chain.map((v) => centers[v]), at.entry]
    const ids = [e.from, ...chain, e.to]
    const upward = ranks[e.to] < ranks[e.from]
    for (let k = 0; k + 1 < stops.length; k++) {
      if (stops[k] === stops[k + 1]) continue
      jogs.push({
        band: upward ? ranks[e.from] - 1 - k : ranks[e.from] + k,
        at: stops[k + 1],
        start: Math.min(stops[k], stops[k + 1]),
        end: Math.max(stops[k], stops[k + 1]),
        from: ids[k],
        to: ids[k + 1],
        edge: i,
        up: [upward ? stops[k + 1] : stops[k]],
        down: [upward ? stops[k] : stops[k + 1]],
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

/**
 * A chain column that coincides with a port column of the rank above or
 * below would share cells with that port's vertical inside the band (a
 * forward exit at the centre of the box above; a back exit beside centre
 * in the box below, which climbs past the forward tracks). Nudge
 * such a chain node by a cell where the gaps to its neighbours allow.
 */
function clearPorts(
  graph: Graph,
  layered: Layered,
  centers: number[],
  size: number[],
  backExit: (node: number) => number[],
): void {
  const n = graph.nodes.length
  const ends = new Map<number, number[]>()
  graph.edges.forEach((e, i) => {
    for (const v of layered.chains[i]) ends.set(v, [...(ends.get(v) ?? []), e.from, e.to])
  })
  layered.layers.forEach((row, r) => {
    /** Port column → the node owning it; a chain's own endpoints are no conflict. */
    const ports = new Map<number, number[]>()
    const claim = (col: number, u: number): void => {
      const owners = ports.get(col)
      if (owners) owners.push(u)
      else ports.set(col, [u])
    }
    for (const u of layered.layers[r - 1] ?? []) if (u < n) claim(centers[u], u)
    for (const u of layered.layers[r + 1] ?? []) {
      if (u < n) for (const col of backExit(u)) claim(col, u)
    }
    row.forEach((v, i) => {
      if (v < n) return
      const own: number[] = ends.get(v) ?? []
      const blocked = (col: number): boolean =>
        (ports.get(col) ?? []).some((u) => !own.includes(u))
      if (!blocked(centers[v])) return
      const left = row[i - 1]
      const right = row[i + 1]
      const lo = left === undefined ? 0 : centers[left] + Math.ceil(size[left] / 2) + 1
      const hi = right === undefined ? Number.MAX_SAFE_INTEGER : centers[right] - Math.ceil(size[right] / 2) - 1
      for (const d of [1, -1, 2, -2]) {
        const c = centers[v] + d
        if (c >= lo && c <= hi && !blocked(c)) {
          centers[v] = c
          return
        }
      }
    })
  })
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
  // Arrival labels hang right of a box's entry heads (forward: above the
  // top, back: below the bottom), on rows a chain column passes through: a
  // chain placed right of a box keeps clear of them.
  const headPad = (skip: (i: number) => boolean): number[] => {
    const pad = new Array<number>(layered.up.length).fill(0)
    graph.edges.forEach((e, i) => {
      if (e.from === e.to || skip(i)) return
      const parts = ranks[e.to] > ranks[e.from] ? [e.label, e.cardTo] : [edgeText(e)]
      for (const part of parts) {
        if (part != null) pad[e.to] = Math.max(pad[e.to], Math.min(stringWidth(part), MAX_LABEL) + 1)
      }
    })
    return pad
  }
  // A back edge leaves the source's top and enters the target's bottom two
  // cells off centre, clear of the forward exits and arrivals that own the
  // centre column — the short return arrow mermaid draws. Which side: the
  // port's arm climbs (drops) through the band's forward bus rows, crossing
  // every one that spans the port column; and at the target, the jog from
  // the port toward the route's next stop crosses the target's own exit
  // column when the stop lies on the other side (at the source the
  // arrivals' arms end below the back rows, so its jog crosses nothing).
  // Take the side that costs less.
  // The chain, aligned with an endpoint's centre by Brandes–Köpf, then
  // shifts to that endpoint's port so it runs straight from it.
  const isBack = (e: Edge): boolean => e.from !== e.to && ranks[e.to] < ranks[e.from]
  const allAtHead = headPad(() => false)
  const first = assignPositions(layered, sizes.layW, GAP_X, (node) => allAtHead[node])
  // An edge with a chain carries its label beside the chain's vertical
  // (dagre's label dummy), on whichever chain node has slack enough beside
  // it in the first placement, nearest the middle; the node then reserves
  // that width. Without such slack the label stays at the head row, where
  // it shares the target's row and costs nothing.
  const chainLabel = new Map<number, { edge: number; w: number; side: number }>()
  const labelNode = new Array<number>(graph.edges.length).fill(-1)
  const layerOf = new Array<number>(layered.up.length).fill(0)
  layered.layers.forEach((row, r) => {
    for (const v of row) layerOf[v] = r
  })
  const extent = (v: number): number => (v < graph.nodes.length ? sizes.layW[v] : 1)
  const slack = (v: number, side: number): number => {
    const row = layered.layers[layerOf[v]]
    const u = row[row.indexOf(v) + side]
    if (u === undefined) return 0
    const reserved = side < 0 ? allAtHead[u] : 0
    return Math.abs(first[u] - first[v]) - half(extent(u)) - half(extent(v)) - 1 - reserved
  }
  graph.edges.forEach((e, i) => {
    const chain = layered.chains[i]
    const text = edgeText(e)
    if (chain.length === 0 || text === null) return
    const w = Math.min(stringWidth(text), MAX_LABEL)
    const mid = chain.length >> 1
    let best: { v: number; side: number; dist: number } | null = null
    chain.forEach((v, k) => {
      for (const side of [1, -1]) {
        if (slack(v, side) < w + 1 || chainLabel.has(v) || layered.shared.has(v)) continue
        const dist = Math.abs(k - mid)
        if (best === null || dist < best.dist) best = { v, side, dist }
      }
    })
    if (best === null) return
    const { v, side } = best as { v: number; side: number }
    labelNode[i] = v
    chainLabel.set(v, { edge: i, w, side })
  })
  const labelPad = headPad((i) => labelNode[i] !== -1)
  const labelPadLeft = new Array<number>(layered.up.length).fill(0)
  for (const [v, { w, side }] of chainLabel) {
    if (side > 0) labelPad[v] = w + 1
    else labelPadLeft[v] = w + 1
  }
  /** Forward bus rows in the band below rank `r` whose span covers column `p`. */
  const busOver = (r: number, p: number): number =>
    graph.edges.filter((e) => {
      if (e.from === e.to || ranks[e.from] !== r || ranks[e.to] !== r + 1) return false
      const [a, b] = [first[e.from], first[e.to]]
      return Math.abs(a - b) > 1 && Math.min(a, b) < p && p < Math.max(a, b)
    }).length
  const portSide = (node: number, band: number, toward: number, atTarget: boolean): number => {
    const cx = first[node]
    const cost = (side: number): number => {
      const p = cx + 2 * side
      const exits = graph.edges.some((e) => e.from === node && ranks[e.to] > ranks[node])
      const jog = atTarget && exits && (toward - cx) * side < 0 ? 1 : 0
      return busOver(band, p) + jog
    }
    return cost(-1) < cost(1) ? -1 : 1
  }
  const exitSide = graph.edges.map((e, i) =>
    isBack(e) ? portSide(e.from, ranks[e.from] - 1, first[layered.chains[i][0] ?? e.to], false) : 0,
  )
  const entrySide = graph.edges.map((e, i) => {
    if (!isBack(e)) return 0
    const last = layered.chains[i].at(-1)
    const toward = last === undefined ? first[e.from] + 2 * exitSide[i] : first[last]
    return portSide(e.to, ranks[e.to], toward, true)
  })
  const shift = new Map<number, number>()
  graph.edges.forEach((e, i) => {
    const chain = layered.chains[i]
    if (!isBack(e) || chain.length === 0) return
    const side =
      first[chain[0]] === first[e.from]
        ? exitSide[i]
        : first[chain[chain.length - 1]] === first[e.to]
          ? entrySide[i]
          : 0
    for (const v of chain) shift.set(v, 2 * side)
  })
  const centers = assignPositions(
    layered,
    sizes.layW,
    GAP_X,
    (node) => labelPad[node],
    (v) => shift.get(v) ?? 0,
    (node) => labelPadLeft[node],
  )
  const boxL = (j: number): number => sat(centers[j], half(sizes.boxW[j]))
  const boxR = (j: number): number => boxL(j) + sizes.boxW[j] - 1
  const port = (node: number, side: number): number =>
    Math.max(boxL(node) + 1, Math.min(boxR(node) - 1, centers[node] + 2 * side))
  // A return entering on the left labels leftward; give the leftmost such
  // label room before the first column.
  let margin = 0
  graph.edges.forEach((e, i) => {
    const text = edgeText(e)
    if (!isBack(e) || entrySide[i] >= 0 || text === null || labelNode[i] !== -1) return
    margin = Math.max(margin, Math.min(stringWidth(text), MAX_LABEL) + 1 - port(e.to, -1))
  })
  for (let v = 0; v < centers.length; v++) centers[v] += margin
  clearPorts(graph, layered, centers, sizes.layW, (node) =>
    graph.edges.flatMap((e, i) => (isBack(e) && e.from === node ? [port(node, exitSide[i])] : [])),
  )

  // Top-entry geometry, derivable before placement. A node's entries land
  // across the box top in the order they arrive from (a forward by its
  // source's column, a skip by the column its chain comes down), so no
  // approach crosses another on the way in. A forward arrival whose source
  // sits over the box top keeps its own head and drops straight, unless
  // forwards jog in from both sides of it (their shared bus would cross
  // the drop); the forwards jogging in from outside merge into one
  // arrival, placed at their sources' mean; each skip gets its own. Whatever falls outside the top
  // spreads over the room left beside the straight drops. A label that
  // does not fit before the next entry renders left of its arrow.
  const isSkip = (e: Edge): boolean => e.from !== e.to && ranks[e.to] - ranks[e.from] > 1
  const isFwd = (e: Edge): boolean => e.from !== e.to && ranks[e.to] === ranks[e.from] + 1
  const edgeEntryX = new Array<number>(graph.edges.length).fill(-1)
  const edgeStraight = new Array<boolean>(graph.edges.length).fill(false)
  const edgeLabelLeft = new Array<boolean>(graph.edges.length).fill(false)
  const labelW = (e: Edge): number => {
    if (labelNode[graph.edges.indexOf(e)] !== -1) return -1
    const parts = [e.label, e.cardTo].filter((p) => p != null) as string[]
    return parts.length === 0
      ? -1
      : Math.max(...parts.map((p) => Math.min(stringWidth(p), MAX_LABEL)))
  }
  const into: number[][] = graph.nodes.map(() => [])
  graph.edges.forEach((e, i) => {
    if (isSkip(e) || isFwd(e)) into[e.to].push(i)
  })
  graph.nodes.forEach((_, t) => {
    const entries = into[t]
    if (entries.length === 0) return
    const cx = centers[t]
    const left = boxL(t)
    const right = boxR(t)
    const arrives = (i: number): number => centers[layered.chains[i].at(-1) ?? graph.edges[i].from]
    type Item = { slot: number; w: number; edge: number; key: number }
    const over = (i: number): boolean => {
      const k = arrives(i)
      return k > left && k < right
    }
    const fwdCols = entries.filter((i) => isFwd(graph.edges[i])).map(arrives)
    const flanked = (i: number): boolean =>
      fwdCols.some((c) => c <= left) && fwdCols.some((c) => c >= right)
    const jogging = entries.filter((i) => isFwd(graph.edges[i]) && (!over(i) || flanked(i)))
    let items: Item[] = entries
      .filter((i) => !jogging.includes(i))
      .map((i) => ({ slot: 0, w: labelW(graph.edges[i]), edge: i, key: arrives(i) }))
    if (jogging.length > 0) {
      items.push({
        slot: 0,
        w: Math.max(...jogging.map((i) => labelW(graph.edges[i]))),
        edge: -1,
        key: jogging.reduce((a, i) => a + centers[graph.edges[i].from], 0) / jogging.length,
      })
    }
    // Slots: an arrival at most a cell off centre snaps to it (routeForward
    // straightens such a jog), other in-range arrivals keep their column,
    // the rest spread evenly over the top. Then walk left to right with a
    // cursor over the free head-row cells: each entry lands at its slot
    // (or past the previous label), its own label going right when the
    // next slot leaves room, else left when the cells behind the cursor
    // allow. Null when the top runs out of room.
    const walk = (list: Item[]): { cols: number[]; lefts: boolean[] } | null => {
      list.sort((a, b) => a.key - b.key || a.edge - b.edge)
      const fixed = list.filter((it) => it.key > left && it.key < right)
      for (const item of fixed) item.slot = Math.abs(item.key - cx) <= 1 ? cx : item.key
      const spread = (group: Item[], lo: number, hi: number): void => {
        group.forEach((item, i) => {
          const at = lo + Math.round(((hi - lo) * (i + 1)) / (group.length + 1))
          item.slot = Math.max(left + 1, Math.min(right - 1, at))
        })
      }
      spread(
        list.filter((it) => it.key <= left),
        left,
        fixed.length > 0 ? Math.max(left, fixed[0].slot - 2) : right,
      )
      spread(
        list.filter((it) => it.key >= right),
        fixed.length > 0 ? Math.min(right, fixed[fixed.length - 1].slot + 2) : left,
        right,
      )
      const cols: number[] = []
      const lefts: boolean[] = []
      let cursor = left
      for (const [i, item] of list.entries()) {
        const x = Math.max(item.slot, cursor)
        if (x > right - 1) return null
        const next = list[i + 1]?.slot ?? Number.MAX_SAFE_INTEGER
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
      return { cols, lefts }
    }
    const fwds = entries.filter((i) => isFwd(graph.edges[i]))
    let fit = walk(items)
    // No room for a head each: every forward merges into one arrival on
    // the centre and the skips spread around it.
    if (fit === null && fwds.length > jogging.length) {
      items = [
        ...items.filter((it) => it.edge !== -1 && !isFwd(graph.edges[it.edge])),
        { slot: 0, w: Math.max(...fwds.map((i) => labelW(graph.edges[i]))), edge: -1, key: cx },
      ]
      fit = walk(items)
    }
    if (fit !== null) {
      items.forEach((item, i) => {
        for (const ei of item.edge === -1 ? (fwds.length > jogging.length && fit.cols.length === items.length && items.some((it) => it.edge === -1 && it.key === cx) ? fwds : jogging) : [item.edge]) {
          edgeEntryX[ei] = fit.cols[i]
          edgeLabelLeft[ei] = fit.lefts[i]
        }
      })
      return
    }
    // Legacy: forwards merge on the centre; a skip lands past the arrival
    // labels, or left of centre with its own label flipped left.
    const reach = fwds.length > 0 ? Math.max(cx, ...fwds.map((i) => cx + 1 + labelW(graph.edges[i]))) : -1
    for (const si of entries) {
      if (isFwd(graph.edges[si])) {
        edgeEntryX[si] = cx
        continue
      }
      // A gap of one cell keeps two heads apart; with no room for that
      // on either side, the skip merges onto the centre arrow.
      const clear = reach === -1 ? cx + 2 : reach + 2
      if (clear <= right - 1) edgeEntryX[si] = clear
      else if (cx - 2 >= left + 1) {
        edgeEntryX[si] = cx - 2
        edgeLabelLeft[si] = true
      } else edgeEntryX[si] = cx
    }
  })
  // Every skip and back edge runs through the interior along the column its
  // virtual chain reserved; each band it jogs in lends it a bus track. A
  // skip's departure jog shares the source's fan row (endpoint sharing), so
  // a node's forward fan and its skips split from one `┴` origin.
  graph.edges.forEach((e, i) => {
    if (!isBack(e)) return
    edgeEntryX[i] = port(e.to, entrySide[i])
    edgeLabelLeft[i] = entrySide[i] < 0
  })
  const edgeExitX = new Array<number>(graph.edges.length).fill(-1)
  graph.edges.forEach((e, i) => {
    if (!isBack(e)) return
    // A one-column step reads as a kink; snap the exit to the next stop.
    const next = layered.chains[i].length > 0 ? centers[layered.chains[i][0]] : edgeEntryX[i]
    const exit = port(e.from, exitSide[i])
    edgeExitX[i] = Math.abs(exit - next) <= 1 ? next : exit
  })
  const jogs = chainJogs(graph, ranks, layered, centers, (e, i) => {
    if (isSkip(e)) return { exit: centers[e.from], entry: edgeEntryX[i] }
    return isBack(e) ? { exit: edgeExitX[i], entry: edgeEntryX[i] } : null
  })
  for (const j of jogs) edgeStraight[j.edge] = true
  graph.edges.forEach((e, i) => {
    if (isBack(e)) edgeStraight[i] = true
  })
  const jogTrack = new Map<ChainJog, number>()

  const edgeBus = new Array<number>(graph.edges.length).fill(0)
  const busTracks = new Array<number>(maxRank + 1).fill(0)
  for (let r = 0; r < maxRank; r++) {
    const spans = busSpans(graph, ranks, centers, r, false, (i) =>
      edgeEntryX[i] === -1 ? centers[graph.edges[i].to] : edgeEntryX[i],
    )
    const bandJogs = jogs.filter((j) => j.band === r)
    spans.push(...bandJogs)
    if (spans.length === 0) continue
    // Back-edge arrowheads sit on the first band row, back buses right under
    // it, forward buses below those: with the attach columns offset right of
    // centre, a reciprocal pair then runs as two parallel staircases whose
    // verticals fall outside each other's horizontal spans — no crossings.
    const back = spans.filter((s) => isBack(graph.edges[s.edge]))
    const fwd = spans.filter((s) => !isBack(graph.edges[s.edge]))
    const base = graph.edges.some((e) => isBack(e) && ranks[e.to] === r) ? 1 : 0
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

  const edgeLabelAt = graph.edges.map((_, i) => {
    const v = labelNode[i]
    if (v === -1) return null
    const { w, side } = chainLabel.get(v) as { w: number; side: number }
    const r = layerOf[v]
    return { row: rankY[r] + half(rankH[r]), x: side > 0 ? centers[v] + 2 : centers[v] - 1 - w }
  })
  let contentW = diagramW
  for (const e of graph.edges) {
    if (e.from === e.to) continue
    const at = edgeLabelAt[graph.edges.indexOf(e)]
    if (at !== null) {
      contentW = Math.max(contentW, at.x + (chainLabel.get(labelNode[graph.edges.indexOf(e)])?.w ?? 0))
    } else if (ranks[e.to] > ranks[e.from]) {
      const parts = [e.label, e.cardTo].filter((part) => part != null) as string[]
      const entry = Math.max(placed[e.to].cx, edgeEntryX[graph.edges.indexOf(e)])
      for (const part of parts) {
        const lw = Math.min(stringWidth(part), MAX_LABEL)
        contentW = Math.max(contentW, entry + 2 + lw)
      }
      if (e.cardFrom !== undefined) {
        contentW = Math.max(contentW, placed[e.from].cx + 2 + stringWidth(e.cardFrom))
      }
    } else {
      const text = edgeText(e)
      if (text !== null) {
        // routeBackChain starts the label right of the entry column.
        contentW = Math.max(contentW, edgeEntryX[graph.edges.indexOf(e)] + 2 + Math.min(stringWidth(text), MAX_LABEL))
      }
    }
  }

  return {
    canvasW: contentW,
    canvasH,
    bandEnd,
    rankStart,
    edgeBus,
    laneBase: 0,
    edgeLane: new Array<number>(graph.edges.length).fill(0),
    edgeEntryX,
    edgeExitX,
    edgeStraight,
    skipRoute,
    edgeLabelLeft,
    edgeLabelAt,
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
  // A skip whose target row crosses no box on any intermediate rank runs
  // straight through the diagram into the target's left side; otherwise
  // the bottom lane. (No chains here: LR back edges must lane, and a
  // diagram mixing interior skips with laned returns crosses itself.)
  const edgeStraight = new Array<boolean>(graph.edges.length).fill(false)
  const clearRow = (e: Edge): boolean =>
    !stubRows.has(centers[e.to]) &&
    graph.nodes.every(
      (_, j) =>
        ranks[j] <= ranks[e.from] ||
        ranks[j] >= ranks[e.to] ||
        Math.abs(centers[j] - centers[e.to]) > half(sizes.boxH[j] + sizes.extraH[j]),
    )
  const entryY = graph.edges.map((e) => (isSkip(e) && clearRow(e) ? centers[e.to] : -1))
  const jogs = chainJogs(graph, ranks, layered, centers, (e, i) =>
    entryY[i] === -1 ? null : { exit: centers[e.from], entry: entryY[i] },
  )
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
    const spans = busSpans(graph, ranks, centers, r, true)
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
  const lanes = laneSpans(graph, ranks, placed).filter((s) => !edgeStraight[s.edge])
  let canvasH = diagramH
  let laneBase = 0
  if (lanes.length > 0) {
    const { assigned, count } = packTracks(lanes)
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
    edgeExitX: new Array<number>(graph.edges.length).fill(-1),
    edgeStraight,
    skipRoute,
    edgeLabelLeft: new Array<boolean>(graph.edges.length).fill(false),
    edgeLabelAt: graph.edges.map(() => null),
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
  // Top-down routes every edge through the interior. Left-to-right boxes
  // are three rows tall, leaving no port off the centre row for a return,
  // so LR back edges go around in a lane below — and skips with them, as a
  // diagram mixing interior skips with laned returns crosses itself. Lane
  // endpoints go last within the rank, or whatever the ordering put beyond
  // them would sit in that corridor and be cut through.
  const vertical = graph.dir === 'down' || graph.dir === 'up'
  const interior = (): boolean => vertical
  const inLane = new Array<boolean>(graph.nodes.length).fill(false)
  for (const e of graph.edges) {
    if (e.from !== e.to && ranks[e.to] !== ranks[e.from] + 1 && !vertical) {
      inLane[e.from] = true
      inLane[e.to] = true
    }
  }
  const layered = orderRanks(byRank, graph.edges, ranks, interior, inLane)

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
    const bus = plan.bandEnd[from.rank] + plan.edgeBus[i]
    const lane = plan.laneBase + plan.edgeLane[i]
    if (vertical) {
      if (adjacent)
        routeForward(canvas, from, to, edge, bus, plan.edgeEntryX[i], plan.edgeLabelLeft[i])
      else if (to.rank > from.rank) {
        routeSkip(canvas, from, to, edge, plan.edgeEntryX[i], plan.skipRoute[i], plan.edgeLabelLeft[i], plan.edgeLabelAt[i])
      } else {
        routeBackChain(canvas, from, to, edge, plan.edgeExitX[i], plan.edgeEntryX[i], plan.skipRoute[i], plan.edgeLabelLeft[i], plan.edgeLabelAt[i])
      }
    } else if (adjacent) {
      routeForwardLr(canvas, from, to, edge, bus)
    } else if (to.rank > from.rank && plan.edgeStraight[i]) {
      routeSkipLr(canvas, from, to, edge, plan.skipRoute[i])
    } else {
      routeBackLr(canvas, from, to, edge, lane, laneLabels)
    }
  })
  flushLabels(canvas)
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
 * Back edge, top-down: up out of the source's top, along the column its
 * virtual chain reserved (jogging on a bus row wherever it steps), arrow
 * into the target's bottom. Adjacent returns have no chain and jog once.
 */
function routeBackChain(
  canvas: Canvas,
  from: Placed,
  to: Placed,
  edge: Edge,
  exitX: number,
  entryX: number,
  route: { bus: number; at: number }[],
  labelLeft: boolean,
  labelAt: { row: number; x: number } | null,
): void {
  const fy = from.y
  const headRow = to.y + to.h

  canvas.junction(exitX, fy, U)
  let x = exitX
  let y = fy
  for (const { bus, at } of route) {
    canvas.segV(x, y, bus)
    canvas.segH(bus, x, at)
    x = at
    y = bus
  }
  canvas.segV(x, y, headRow)

  if (edge.headTo === 'none') canvas.addBits(entryX, headRow, D)
  else canvas.set(entryX, headRow, headGlyph(edge.headTo, '▲'), 'edge')
  if (edge.headFrom !== 'none') canvas.set(exitX, fy, headGlyph(edge.headFrom, '▼'), 'edge')

  const text = edgeText(edge)
  if (text !== null && labelAt !== null) placeLabel(canvas, text, labelAt.row, labelAt.x)
  else if (text !== null) {
    const start = labelLeft ? sat(entryX, Math.min(stringWidth(text), MAX_LABEL)) : entryX + 1
    placeLabel(canvas, text, headRow, start)
  }
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
  labelAt: { row: number; x: number } | null,
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
  if (text !== null && labelAt !== null) placeLabel(canvas, text, labelAt.row, labelAt.x)
  else if (text !== null) {
    const start = labelLeft ? sat(entryX, Math.min(stringWidth(text), MAX_LABEL)) : entryX + 1
    placeLabel(canvas, text, headRow, start)
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

/**
 * Edge labels queued while routing and written once every route has
 * landed (`flushLabels`), so a label knows what it sits on: it interrupts
 * a line the way a lane label interrupts its lane (the text sits across the
 * line, which stays readable either side — a cardinality on its own bus
 * row, a label across a return climbing past it) and stops short of a box
 * or other text.
 */
let pendingLabels: { label: string; row: number; x: number }[] = []

function placeLabel(_canvas: Canvas, label: string, row: number, x: number): void {
  pendingLabels.push({ label, row, x })
}

function flushLabels(canvas: Canvas): void {
  const queued = pendingLabels
  pendingLabels = []
  for (const { label, row, x } of queued) writeLabel(canvas, label, row, x)
}

function writeLabel(canvas: Canvas, label: string, row: number, startX: number): void {
  if (row >= canvas.h) return
  const text = fitLabel(label, MAX_LABEL)
  let x = startX
  for (const [c, cw] of measured(text)) {
    if (cw === 0) continue
    if (x + cw > canvas.w) break
    let blocked = false
    for (let k = 0; k < cw; k++) {
      const i = canvas.idx(x + k, row)
      if (canvas.ch[i] !== ' ' || canvas.occupied[i]) blocked = true
    }
    if (blocked) break
    for (let k = 0; k < cw; k++) canvas.mask[canvas.idx(x + k, row)] = 0
    canvas.set(x, row, c, 'edgeLabel')
    for (let k = 1; k < cw; k++) canvas.set(x + k, row, CONT, 'edgeLabel')
    x += cw
  }
}
