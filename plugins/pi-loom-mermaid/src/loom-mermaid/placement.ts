/**
 * Cross-axis coordinate assignment for a layered graph — Brandes and Köpf,
 * "Fast and Simple Horizontal Coordinate Assignment" (2002).
 *
 * Every node is aligned with a median neighbour where that crosses no
 * long-edge chain (a type-1 conflict), aligned nodes form blocks that get
 * one coordinate, and blocks are compacted as tightly as the ordering and
 * the separations allow. Of the four runs (top/bottom × left/right) the one
 * with the straightest segments is kept.
 *
 * Input and output are in the layered graph of `orderRanks`: ids are real
 * nodes and virtual chain nodes alike, `size[v]` is each one's cross-axis
 * extent, and a virtual node is any id at or past `realCount`. The returned
 * centres are non-negative integers that keep `sep(left, right)` cells
 * between each pair of neighbours. `offset(v)` moves a node off its aligned
 * coordinate afterwards — a chain that should line up with a port beside
 * its endpoint's centre rather than the centre itself.
 */

export interface LayeredGraph {
  layers: number[][]
  up: number[][]
  down: number[][]
}

export function brandesKoepf(
  g: LayeredGraph,
  size: number[],
  sep: (left: number, right: number) => number,
  realCount: number,
  offset: (v: number) => number = () => 0,
): number[] {
  const n = g.up.length
  const layerOf = new Array<number>(n).fill(0)
  const pos = new Array<number>(n).fill(0)
  g.layers.forEach((row, r) => {
    row.forEach((v, i) => {
      layerOf[v] = r
      pos[v] = i
    })
  })
  const conflicts = markConflicts(g, pos, realCount)

  const runs: number[][] = []
  const roots: number[][] = []
  for (const fromBottom of [false, true]) {
    for (const fromRight of [false, true]) {
      const layers = fromBottom ? [...g.layers].reverse() : g.layers
      const view: LayeredGraph = {
        layers: fromRight ? layers.map((row) => [...row].reverse()) : layers,
        up: fromBottom ? g.down : g.up,
        down: fromBottom ? g.up : g.down,
      }
      const vpos = new Array<number>(n).fill(0)
      for (const row of view.layers) row.forEach((v, i) => (vpos[v] = i))
      // Mirrored runs see the pair the other way round.
      const gap = fromRight ? (l: number, r: number) => sep(r, l) : sep
      const alignment = alignVertically(view, vpos, conflicts)
      const x = compact(view, vpos, size, gap, alignment)
      runs.push(fromRight ? x.map((c) => -c) : x)
      roots.push(alignment.root)
    }
  }
  const chosen = straightest(runs, g, size, realCount)
  const centers = runs[chosen].map((c, v) => c + offset(v))
  const root = roots[chosen]

  // Rounding, offsets and the class shifts of the compaction can shave a
  // cell off a separation; sweeping each layer left to right restores it,
  // moving a whole aligned block so the run's straight segments stay
  // straight, until every layer holds. Then pin the origin at 0.
  const members = new Map<number, number[]>()
  for (let v = 0; v < n; v++) {
    const list = members.get(root[v])
    if (list) list.push(v)
    else members.set(root[v], [v])
  }
  for (let pass = 0; pass < n; pass++) {
    let moved = false
    for (const row of g.layers) {
      for (let i = 1; i < row.length; i++) {
        const [u, w] = [row[i - 1], row[i]]
        const gap = size[u] / 2 + sep(u, w) + size[w] / 2
        const deficit = centers[u] + gap - centers[w]
        if (deficit <= 0) continue
        for (const m of members.get(root[w]) ?? [w]) centers[m] += deficit
        moved = true
      }
    }
    if (!moved) break
  }
  let min = Number.POSITIVE_INFINITY
  for (const row of g.layers) for (const v of row) min = Math.min(min, centers[v] - size[v] / 2)
  if (!Number.isFinite(min)) min = 0
  return centers.map((c) => Math.max(0, Math.round(c - min)))
}

/**
 * Type-1 conflicts: a segment between a real node and anything crossing an
 * inner segment (virtual to virtual). Inner segments win alignment so long
 * edges stay straight; the crossing segment is barred from aligning.
 */
function markConflicts(g: LayeredGraph, pos: number[], realCount: number): Set<number> {
  const n = g.up.length
  const conflicts = new Set<number>()
  const isVirtual = (v: number): boolean => v >= realCount
  const innerUpper = (v: number): number | null => {
    if (!isVirtual(v)) return null
    const u = g.up[v].find(isVirtual)
    return u === undefined ? null : u
  }
  for (let i = 1; i < g.layers.length; i++) {
    const row = g.layers[i]
    const upper = g.layers[i - 1]
    let k0 = 0
    let l = 0
    for (let l1 = 0; l1 < row.length; l1++) {
      const v = row[l1]
      const inner = innerUpper(v)
      if (l1 !== row.length - 1 && inner === null) continue
      const k1 = inner === null ? upper.length - 1 : pos[inner]
      for (; l <= l1; l++) {
        const w = row[l]
        for (const u of g.up[w]) {
          if ((pos[u] < k0 || pos[u] > k1) && !(isVirtual(u) && isVirtual(w))) {
            conflicts.add(u * n + w)
          }
        }
      }
      k0 = k1
    }
  }
  return conflicts
}

interface Alignment {
  root: number[]
  align: number[]
}

/** Align each node with a median upper neighbour, left to right, no crossings. */
function alignVertically(g: LayeredGraph, pos: number[], conflicts: Set<number>): Alignment {
  const n = g.up.length
  const root = Array.from({ length: n }, (_, v) => v)
  const align = [...root]
  for (let i = 1; i < g.layers.length; i++) {
    let r = -1
    for (const v of g.layers[i]) {
      const ups = [...g.up[v]].sort((a, b) => pos[a] - pos[b])
      const d = ups.length
      if (d === 0) continue
      const medians = d % 2 === 1 ? [ups[(d - 1) / 2]] : [ups[d / 2 - 1], ups[d / 2]]
      for (const u of medians) {
        if (align[v] !== v) break
        if (conflicts.has(u * n + v) || r >= pos[u]) continue
        align[u] = v
        root[v] = root[u]
        align[v] = root[v]
        r = pos[u]
      }
    }
  }
  return { root, align }
}

/** Place blocks as far left as the ordering allows; returns centre per node. */
function compact(
  g: LayeredGraph,
  pos: number[],
  size: number[],
  sep: (left: number, right: number) => number,
  { root, align }: Alignment,
): number[] {
  const n = g.up.length
  const pred = new Array<number>(n).fill(-1)
  for (const row of g.layers) for (let i = 1; i < row.length; i++) pred[row[i]] = row[i - 1]
  const sink = Array.from({ length: n }, (_, v) => v)
  const shift = new Array<number>(n).fill(Number.POSITIVE_INFINITY)
  const x = new Array<number>(n).fill(Number.NaN)
  const delta = (u: number, w: number): number => size[u] / 2 + sep(u, w) + size[w] / 2

  const placeBlock = (v: number): void => {
    if (!Number.isNaN(x[v])) return
    x[v] = 0
    let w = v
    do {
      if (pos[w] > 0) {
        const p = pred[w]
        const u = root[p]
        placeBlock(u)
        if (sink[v] === v) sink[v] = sink[u]
        if (sink[v] !== sink[u]) {
          shift[sink[u]] = Math.min(shift[sink[u]], x[v] - x[u] - delta(p, w))
        } else {
          x[v] = Math.max(x[v], x[u] + delta(p, w))
        }
      }
      w = align[w]
    } while (w !== v)
  }
  for (let v = 0; v < n; v++) if (root[v] === v) placeBlock(v)

  const out = new Array<number>(n)
  for (let v = 0; v < n; v++) {
    out[v] = x[root[v]]
    const s = shift[sink[root[v]]]
    if (Number.isFinite(s)) out[v] += s
  }
  return out
}

/**
 * The run whose segments bend least (weighted sum of cross-axis offsets)
 * among those no wider than the narrowest run plus a fifth. Brandes–Köpf
 * balance the four by averaging medians, which splits the difference
 * wherever the runs disagree and leaves a chain kinked by a cell or two;
 * on a cell grid one consistent run reads better. On a large graph a
 * straighter run can be much wider, and width is what a terminal lacks.
 */
function straightest(
  runs: number[][],
  g: LayeredGraph,
  size: number[],
  realCount: number,
): number {
  // A kinked chain segment costs most (a bus track in every band it jogs
  // through), a kinked edge between boxes next; the jog where a long edge
  // leaves or reaches its box is the natural place for it to step aside.
  const weight = (v: number, w: number): number => {
    const virtual = Number(v >= realCount) + Number(w >= realCount)
    return virtual === 2 ? 8 : virtual === 1 ? 1 : 4
  }
  const scored = runs.map((x, index) => {
    let bends = 0
    for (let v = 0; v < g.down.length; v++) {
      for (const w of g.down[v]) bends += weight(v, w) * Math.abs(x[v] - x[w])
    }
    let lo = Number.POSITIVE_INFINITY
    let hi = Number.NEGATIVE_INFINITY
    x.forEach((c, v) => {
      lo = Math.min(lo, c - size[v] / 2)
      hi = Math.max(hi, c + size[v] / 2)
    })
    return { index, bends, width: hi - lo }
  })
  const narrowest = Math.min(...scored.map((s) => s.width))
  let best = scored[0]
  for (const s of scored) {
    if (s.width > narrowest * 1.2) continue
    if (best.width > narrowest * 1.2 || s.bends < best.bends || (s.bends === best.bends && s.width < best.width)) {
      best = s
    }
  }
  return best.index
}
