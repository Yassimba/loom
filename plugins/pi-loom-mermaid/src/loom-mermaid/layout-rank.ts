/**
 * Rank assignment and crossing reduction (Sugiyama).
 */
import type { Edge, Graph } from './graph.ts'
import type { LayeredGraph } from './placement.ts'

// ------------------------------------------------------------------ ranking

/**
 * Rank assignment along the flow axis.
 *
 * Cycles are broken by a DFS colouring pass in declaration order, so the
 * edge treated as the return is the one the author wrote against the flow
 * (`A --> B --> C --> A` returns on `C --> A`); greedy feedback-set
 * heuristics reverse fewer edges, but the orders they pick measure worse
 * on the fixtures than the author's own. Reversed edges take part in
 * ranking in their reversed direction, so a return always climbs at least
 * one rank. Longest-path layering puts each
 * node as early as its predecessors allow, then Nikolov's node promotion
 * (mirrored: nodes move later) shortens edges while that removes more
 * virtual chain nodes than it adds.
 *
 * Class and ER diagrams mix hierarchy (inheritance, composition,
 * aggregation: a triangle or diamond head) with association. The
 * hierarchy is layered first, on its own; an association then only pulls
 * its target later when the hierarchy left it no later than its source,
 * so it stays a forward edge the router can draw but never stretches a
 * hierarchy that already orders the two (after Gutwenger et al., "A new
 * approach for visualizing UML class diagrams", SoftVis 2003). Diagrams
 * without hierarchy heads rank every edge alike.
 */
export function computeRanks(graph: Graph): number[] {
  const hierarchy = (e: Edge): boolean =>
    e.headTo === 'triangle' || e.headFrom === 'triangle' || e.headTo.startsWith('diamond') || e.headFrom.startsWith('diamond')
  if (!graph.edges.some(hierarchy) || graph.edges.every(hierarchy)) return rankEdges(graph, graph.edges)
  const strong = rankEdges(graph, graph.edges.filter(hierarchy))
  // Associations the hierarchy already orders forward add no constraint.
  const needed = graph.edges.filter((e) => hierarchy(e) || strong[e.to] <= strong[e.from])
  return rankEdges(graph, needed)
}

function rankEdges(graph: Graph, edges: Edge[]): number[] {
  const n = graph.nodes.length
  const children: number[][] = Array.from({ length: n }, () => [])
  for (const e of edges) {
    if (e.from !== e.to) children[e.from].push(e.to)
  }
  const color = new Uint8Array(n)
  const forward = new Set<string>()
  const postorder: number[] = []
  // Declaration order: the first node the author named is the entry, even
  // when a return edge gives it a predecessor.
  for (let start = 0; start < n; start++) {
    if (color[start] === 0) dfsDag(start, children, color, forward, postorder)
  }
  const order = postorder.reverse()
  const succ: number[][] = Array.from({ length: n }, () => [])
  const pred: number[][] = Array.from({ length: n }, () => [])
  for (const e of edges) {
    if (e.from === e.to) continue
    const [a, b] = forward.has(`${e.from}>${e.to}`) ? [e.from, e.to] : [e.to, e.from]
    succ[a].push(b)
    pred[b].push(a)
  }

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
  return rank
}

/** Iterative DFS recording postorder and skipping edges back into the stack. */
function dfsDag(
  start: number,
  children: number[][],
  color: Uint8Array,
  forward: Set<string>,
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
      forward.add(`${u}>${v}`)
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
  /** Virtual nodes of returns: columns drawn beside the boxes, not among them. */
  lanes: Set<number>
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
      const key = k <= headEnd ? `f${e.from}@${r}` : k >= tailStart ? `t${e.to}@${r}` : null
      let v = key === null ? undefined : trunks.get(key)
      if (v === undefined) {
        v = up.length
        up.push([])
        down.push([])
        layers[r].push(v)
        if (key !== null) trunks.set(key, v)
      } else shared.add(v)
      chains[i].push(v)
      link(prev, v, upward)
      prev = v
    }
    link(prev, e.to, upward)
  })
  return { layers, up, down, chains, shared, lanes: new Set<number>() }
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
 * Returns are pinned to one end of every rank they cross, `trailing`
 * nodes to the last position.
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
  const layered = normalize(byRank, edges, ranks, interior)
  // A return climbs a column of its own. Ordered among the boxes it runs
  // between a fan and that fan's targets: both meet at the node the return
  // enters, so the layered count sees no crossing where the drawing has
  // one (the fan leaves along a bus row, the return arrives on it). The
  // column is pinned to the end of the rank its own source sits toward,
  // which leaves every box between the return's ends on one hand.
  const outside = new Map<number, number>()
  edges.forEach((e, i) => {
    if (e.from === e.to || ranks[e.to] >= ranks[e.from]) return
    for (const v of layered.chains[i]) {
      outside.set(v, e.from)
      layered.lanes.add(v)
    }
  })
  const isTrailing = (v: number): boolean => trailing[v] ?? false
  const partition = (row: number[]): void => {
    const side = (v: number, i: number): number => {
      const src = outside.get(v)
      if (src === undefined) return 0
      const own = byRank[ranks[src]]
      const at = own.indexOf(src)
      return (at < 0 ? i * 2 : at * 2) < (at < 0 ? row.length : own.length) ? -1 : 1
    }
    const keyed = row.map((v, i) => ({ v, k: side(v, i), t: Number(isTrailing(v)) }))
    keyed.sort((a, b) => a.k - b.k || a.t - b.t)
    for (let i = 0; i < keyed.length; i++) row[i] = keyed[i].v
  }
  for (const row of byRank) partition(row)
  for (const row of layered.layers) partition(row)
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
      const score = total()
      if (score < current) {
        current = score
        stale = 0
      } else stale++
      if (score < bestCrossings) {
        bestCrossings = score
        best = layers.map((row) => [...row])
      }
    }
  }
  // The sweeps settle into a local minimum shaped by the starting order:
  // declaration order first, then a few seeded shuffles, best kept. The
  // author's statement order is intent, not noise, and keeping it costs
  // no crossings on average (Domrös and von Hanxleden, "Model Order in
  // Sugiyama Layouts", GD 2022): every tie-break here is stable, and a
  // shuffle replaces it only on a strictly lower crossing count.
  let seed = 0x9e3779b9
  const random = (): number => {
    seed = (Math.imul(seed, 1103515245) + 12345) >>> 0
    return seed / 0x100000000
  }
  const restarts = n < 40 ? 12 : 4
  for (let restart = 0; restart < restarts && bestCrossings > 0; restart++) {
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
