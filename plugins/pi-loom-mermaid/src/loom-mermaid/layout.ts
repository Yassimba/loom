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

import type { Canvas } from './canvas.ts'
import type { Anchor, Edge, LineKind } from './graph.ts'
import type { Graph } from './graph.ts'
import { fitLabel, type Limits, wrapLabel } from './labels.ts'
import {
  edgeText,
  GAP_X,
  GAP_Y,
  half,
  labelCols,
  labelStart,
  MAX_CANVAS_CELLS,
  PAD,
  sat,
  type Placed,
} from './layout-geom.ts'
import { computeRanks, type Layered, orderRanks } from './layout-rank.ts'
import {
  assignPositions,
  assignTracks,
  bicliqueKeys,
  busSpans,
  type ChainJog,
  chainJogs,
  clearPorts,
  laneSpans,
  packTracks,
  skipRoutes,
  transitiveEdges,
} from './layout-tracks.ts'
import { stringWidth } from './width.ts'

export { PAD, MAX_CANVAS_CELLS, sat, half, edgeText, type Placed } from './layout-geom.ts'
export { orderRanks } from './layout-rank.ts'

/** Per-node dimensions. `layW` includes room for self-edge hooks and labels. */
interface NodeSizes {
  boxW: number[]
  boxH: number[]
  layW: number[]
  layH: number[]
  selfLabelW: number[]
  /** Cells of a frame's top border taken by its title (0 for other boxes). */
  titleW: number[]
  /** Edge labels are fitted to this many columns. */
  maxLabel: number
}

/** What to draw inside a node box. */
export type NodeExtra =
  | { kind: 'plain' }
  | { kind: 'frame'; sub: Canvas }
  | { kind: 'compartments'; sections: string[][] }

/**
 * One edge's path as data: cell corners from the source border to the head
 * cell, in drawing order. Painting derives everything else — the junction
 * bits at the border, the head and tail glyphs from the approach direction,
 * the segments between corners. Labels wait until every route has landed.
 */
/** Where a self-loop hooks its box: off a bottom corner, or a stub below. */
export type Hook = 'left' | 'right' | 'below'

export interface Route {
  points: [number, number][]
  /** Self-loops only (no points): which hook to draw. */
  hook?: Hook
  /** Labels written across the lines once all routes are drawn. */
  labels: { text: string; row: number; x: number }[]
  /** A lane label that slides along its run to a clear stretch (left-to-right lanes). */
  laneLabel?: LaneLabel
  /**
   * Cells needing junction bits beyond what the segments give: frame border
   * cells pierced on the way to an inner node (`v` / `h`, the run's
   * direction there) and bundle joins where a straight run meets the shared
   * bus (`j`, so it reads as a tee rather than a hop).
   */
  through?: Through[]
}

/**
 * A cross-frame edge end resolved against the frame's contents: the box the
 * route leaves or enters (frame-local coordinates, the frame's corner at
 * 0,0) and the border cells crossed. Preferably the inner node itself, with
 * a straight stub to the frame border through blank padding or nested
 * borders; else the frame border at the inner node's column or row. Ports
 * on one side of one frame never share a coordinate: a second edge is
 * nudged a cell over, inside its box, else falls back to the frame border.
 */
type Through = [number, number, 'v' | 'h' | 'j']

interface Port {
  box: Omit<Placed, 'rank'>
  through: Through[]
  /** The inner node's own coordinate on this side, before any nudge. */
  wanted: number
}

type Side = 'top' | 'bottom' | 'left' | 'right'

function framePort(
  sub: Canvas,
  fw: number,
  fh: number,
  titleW: number,
  a: Anchor,
  side: Side,
  used: Set<string>,
): Port {
  const ox = 1 + half(fw - 2 - sub.w)
  const oy = 1 + half(fh - 2 - sub.h)
  const ax = a.x + ox
  const ay = a.y + oy
  const vertical = side === 'bottom' || side === 'top'
  const anchorC = vertical ? ax + half(a.w) : ay + half(a.h)
  // The top border carries the title; no port lands on it.
  const nearest = (lo: number, hi: number): number[] => {
    const out: number[] = []
    for (let d = 0; d <= hi - lo; d++) {
      if (anchorC + d <= hi) out.push(anchorC + d)
      if (d > 0 && anchorC - d >= lo) out.push(anchorC - d)
    }
    return out.filter((c) => !used.has(`${side}:${c}`) && (side !== 'top' || c > titleW))
  }
  // Walk from the cell past the inner box to the cell inside the frame
  // border; blank cells pass, a nested frame border is pierced. A box within
  // a cell of the border takes no stub: a one-cell pierce reads as noise,
  // the border port beside it says the same.
  const stub = (c: number): Through[] | null => {
    const through: Through[] = []
    const [from, to] =
      side === 'bottom'
        ? [ay + a.h, fh - 2]
        : side === 'top'
          ? [ay - 1, 1]
          : side === 'right'
            ? [ax + a.w, fw - 2]
            : [ax - 1, 1]
    const step = side === 'left' || side === 'top' ? -1 : 1
    if ((to - from) * step < 1) return null
    for (let k = from; k !== to + step; k += step) {
      const [x, y] = vertical ? [c - ox, k - oy] : [k - ox, c - oy]
      if (x < 0 || y < 0 || x >= sub.w || y >= sub.h) continue
      const i = sub.idx(x, y)
      if (!sub.occupied[i] && sub.ch[i] === ' ') continue
      // The first cell holds the arrowhead, so it must be free.
      if (k !== from && sub.role[i] === 'border' && sub.ch[i] === (vertical ? '─' : '│')) {
        through.push(vertical ? [c, k, 'v'] : [k, c, 'h'])
        continue
      }
      return null
    }
    const border = side === 'bottom' ? fh - 1 : side === 'right' ? fw - 1 : 0
    through.push(vertical ? [c, border, 'v'] : [border, c, 'h'])
    return through
  }
  for (const c of vertical ? nearest(ax + 1, ax + a.w - 2) : nearest(ay + 1, ay + a.h - 2)) {
    const through = stub(c)
    if (through === null) continue
    used.add(`${side}:${c}`)
    const box = { x: ax, y: ay, w: a.w, h: a.h, cx: ax + half(a.w), cy: ay + half(a.h) }
    if (vertical) box.cx = c
    else box.cy = c
    return { box, through, wanted: anchorC }
  }
  const [c] = vertical ? nearest(1, fw - 2) : nearest(1, fh - 2)
  const at = c ?? anchorC
  used.add(`${side}:${at}`)
  const box = { x: 0, y: 0, w: fw, h: fh, cx: half(fw), cy: half(fh) }
  if (vertical) box.cx = at
  else box.cy = at
  return { box, through: [], wanted: anchorC }
}

/** A port translated to where its frame landed. */
function portAt(port: Port, frame: Placed): Placed & { through: Through[] } {
  const b = port.box
  return {
    x: frame.x + b.x,
    y: frame.y + b.y,
    w: b.w,
    h: b.h,
    cx: frame.x + b.cx,
    cy: frame.y + b.cy,
    rank: frame.rank,
    through: port.through.map(([x, y, k]) => [frame.x + x, frame.y + y, k]),
  }
}

/** A lane label waiting for every route to land before claiming its spot. */
export interface LaneLabel {
  text: string
  y: number
  lo: number
  hi: number
}

/** The placement stage's result: canvas size and a route per edge (`null` for a self loop, which draws its own stub). */
interface Plan {
  canvasW: number
  canvasH: number
  routes: Route[]
}

function placeTd(
  ranks: number[],
  maxRank: number,
  byRank: number[][],
  layered: Layered,
  sizes: NodeSizes,
  graph: Graph,
  placed: Placed[],
  extras: NodeExtra[],
): Plan {
  const maxLabel = sizes.maxLabel
  // Arrival labels hang right of a box's entry heads (forward: above the
  // top, back: below the bottom), on rows a chain column passes through: a
  // chain placed right of a box keeps clear of them.
  const headPad = (skip: (i: number) => boolean): number[] => {
    const pad = new Array<number>(layered.up.length).fill(0)
    graph.edges.forEach((e, i) => {
      if (e.from === e.to || skip(i)) return
      const text = edgeText(e)
      if (text !== null) pad[e.to] = Math.max(pad[e.to], labelCols(text, maxLabel) + 1)
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
  /** Per edge, the chain node carrying its label, the label width and side (+1 right, -1 left). */
  const chainLabel: ({ v: number; w: number; side: number } | null)[] = graph.edges.map(() => null)
  const taken = new Set<number>()
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
    // A neighbour's arrival already names this text at the head; the skip
    // concentrates into that arrival, so a second copy beside the chain
    // would only repeat it.
    const namedAtHead = graph.edges.some(
      (o, k) =>
        k !== i && o.to === e.to && o.from !== o.to && ranks[o.from] < ranks[o.to] && layered.chains[k].length === 0 &&
        edgeText(o) === text && o.line === e.line && o.headTo === e.headTo,
    )
    if (namedAtHead) return
    const w = labelCols(text, maxLabel)
    const mid = chain.length >> 1
    let best: { v: number; side: number; dist: number } | null = null
    // A return leaving its source's side on this row would run under the
    // label: leave that stretch to the leg.
    const onLeg = (v: number, side: number): boolean => {
      const lo = side > 0 ? first[v] + 2 : first[v] - 1 - w
      return graph.edges.some((o, k) => {
        const head = layered.chains[k][0]
        if (!isBack(o) || head === undefined || ranks[o.from] !== layerOf[v] || extras[o.from].kind !== 'plain') return false
        return lo <= Math.max(first[o.from], first[head]) && lo + w > Math.min(first[o.from], first[head])
      })
    }
    chain.forEach((v, k) => {
      for (const side of [1, -1]) {
        if (slack(v, side) < w + 1 || taken.has(v) || layered.shared.has(v) || onLeg(v, side)) continue
        const dist = Math.abs(k - mid)
        if (best === null || dist < best.dist) best = { v, side, dist }
      }
    })
    if (best === null) return
    const { v, side } = best as { v: number; side: number }
    chainLabel[i] = { v, w, side }
    taken.add(v)
  })
  // Plain boxes only (a class box's side rows are compartment rules).
  // Decided on a coordinate set: once on the first placement, to reserve
  // room for a head label on a side leg, and again on the final one.
  const isSkip = (e: Edge): boolean => e.from !== e.to && ranks[e.to] - ranks[e.from] > 1
  // A self-loop hooks the corner away from its box's returns, leaving them
  // the side they come from.
  const hookSide = graph.nodes.map((_, j) =>
    graph.edges.some((e, i) => isBack(e) && e.to === j && first[layered.chains[i].at(-1) ?? e.from] > first[j]) ? -1 : 1,
  )
  // Side ports (port assignment as in Hegemann and Wolff, GD 2023): an
  // edge whose chain runs well outside a plain box, with a clear leg, goes
  // through the facing side — into a target, or out of a return's source —
  // instead of a port beside the box's own stem. One edge per box side.
  const sidePorts = (x: number[]): { entry: number[]; exit: number[] } => {
    const entry = new Array<number>(graph.edges.length).fill(0)
    const exit = new Array<number>(graph.edges.length).fill(0)
    // A self-loop's hook owns one side of its box.
    const taken = new Set<string>(graph.edges.filter((e) => e.from === e.to).map((e) => `${e.to}:${hookSide[e.to]}`))
    const bl = (j: number): number => sat(x[j], half(sizes.boxW[j]))
    const br = (j: number): number => bl(j) + sizes.boxW[j] - 1
    const clearLeg = (j: number, col: number): boolean =>
      byRank[ranks[j]].every((k) => k === j || Math.min(col, x[j]) - 1 >= br(k) || Math.max(col, x[j]) + 1 <= bl(k))
    const sideOf = (j: number, col: number): number => (col <= bl(j) - 3 ? -1 : col >= br(j) + 3 ? 1 : 0)
    // A label beside a chain on the source's row would sit on the leg.
    const labelOnLeg = (j: number, col: number): boolean =>
      chainLabel.some((label) => {
        if (label === null || layerOf[label.v] !== ranks[j]) return false
        const lo = label.side > 0 ? x[label.v] + 2 : x[label.v] - 1 - label.w
        return lo <= Math.max(col, x[j]) && lo + label.w > Math.min(col, x[j])
      })
    graph.edges.forEach((e, i) => {
      if (!isBack(e) || extras[e.from].kind !== 'plain') return
      const head = layered.chains[i][0]
      if (head === undefined) return
      const side = sideOf(e.from, x[head])
      if (side === 0 || taken.has(`${e.from}:${side}`) || !clearLeg(e.from, x[head]) || labelOnLeg(e.from, x[head])) return
      taken.add(`${e.from}:${side}`)
      exit[i] = side
    })
    graph.edges.forEach((e, i) => {
      if (!(isSkip(e) || isBack(e)) || extras[e.to].kind !== 'plain') return
      const last = layered.chains[i].at(-1)
      if (last === undefined) return
      const col = x[last]
      const side = sideOf(e.to, col)
      if (side === 0 || taken.has(`${e.to}:${side}`)) return
      // A skip is a lone detour only when no forward arrival jogs in; a
      // return always takes the side, since its bottom port would sit
      // beside the target's own fan-out stem.
      const othersJog =
        isSkip(e) &&
        graph.edges.some(
          (o, k) =>
            k !== i && o.to === e.to && o.from !== o.to && ranks[o.from] < ranks[o.to] && Math.abs(x[layered.chains[k].at(-1) ?? o.from] - x[e.to]) > 1,
        )
      // A blank cell between the leg and every other box on the rank, or
      // the leg would read as leaving that box.
      if (othersJog || !clearLeg(e.to, col)) return
      taken.add(`${e.to}:${side}`)
      entry[i] = side
    })
    return { entry, exit }
  }
  const labelPad = headPad((i) => chainLabel[i] !== null)
  const labelPadLeft = new Array<number>(layered.up.length).fill(0)
  for (const label of chainLabel) {
    if (label === null) continue
    if (label.side > 0) labelPad[label.v] = label.w + 1
    else labelPadLeft[label.v] = label.w + 1
  }
  // A self-loop's hook and label, four cells past the border on the
  // hook's side (pads count from a cell past the centre).
  graph.nodes.forEach((_, j) => {
    const w = sizes.selfLabelW[j]
    if (w === 0) return
    const beyond = w + 3 + (sizes.boxW[j] - half(sizes.boxW[j]))
    if (hookSide[j] > 0) labelPad[j] = Math.max(labelPad[j], beyond)
    else labelPadLeft[j] = Math.max(labelPadLeft[j], w + 3 + half(sizes.boxW[j]))
  })
  // A side entry's head label rides its leg: the chain's last node keeps
  // that much room on its box side.
  sidePorts(first).entry.forEach((side, i) => {
    const text = edgeText(graph.edges[i])
    if (side === 0 || text === null || chainLabel[i] !== null) return
    const v = layered.chains[i].at(-1) as number
    const w = labelCols(text, maxLabel) + 3
    if (side > 0) labelPadLeft[v] = Math.max(labelPadLeft[v], w)
    else labelPad[v] = Math.max(labelPad[v], w)
  })
  /** Forward bus rows in the band below rank `r` whose span covers column `p`. */
  // Forward buses on band r: an edge's first hop runs from its source to
  // its next stop, the target or the first node of its chain.
  const busOver = (r: number, p: number): number =>
    graph.edges.filter((e, i) => {
      if (e.from === e.to || ranks[e.from] !== r || ranks[e.to] <= r) return false
      const [a, b] = [first[e.from], first[layered.chains[i][0] ?? e.to]]
      return Math.abs(a - b) > 1 && Math.min(a, b) < p && p < Math.max(a, b)
    }).length
  const portSide = (node: number, band: number, toward: number, atTarget: boolean): number => {
    const cx = first[node]
    const cost = (side: number): number => {
      const p = cx + 2 * side
      const exits = graph.edges.some((e) => e.from === node && ranks[e.to] > ranks[node])
      const jog = atTarget && exits && (toward - cx) * side < 0 ? 1 : 0
      // Ties go to the side facing the route's next stop.
      const away = (toward - cx) * side < 0 ? 0.5 : 0
      return busOver(band, p) + jog + away
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
  const place = (): number[] =>
    assignPositions(
      layered,
      sizes.layW,
      GAP_X,
      (node) => labelPad[node],
      (v) => shift.get(v) ?? 0,
      (node) => labelPadLeft[node],
    )
  const extentOf = (v: number): number => (v < graph.nodes.length ? sizes.layW[v] : 1)
  /** Left edge, rightmost box edge, rightmost label end. */
  const extentsOf = (c: number[]): [number, number, number] => {
    let lo = Number.POSITIVE_INFINITY
    let box = 0
    let text = 0
    c.forEach((x, v) => {
      lo = Math.min(lo, x - half(extentOf(v)) - labelPadLeft[v])
      box = Math.max(box, x + extentOf(v) - half(extentOf(v)))
      text = Math.max(text, x + labelPad[v])
    })
    return [lo, box, text]
  }
  let centers = place()
  // A head label reserves room on the right by default. A box
  // whose one labelled arrival would fit on its left instead flips it
  // there when that narrows the drawing (dagre's label dummy, either side).
  const labelFlipped = new Set<number>()
  const single = graph.nodes.map((_, v) => {
    const list = graph.edges.filter((e, i) => e.to === v && e.from !== v && !isBack(e) && chainLabel[i] === null && edgeText(e) !== null)
    if (list.length !== 1 || labelPadLeft[v] !== 0 || sizes.selfLabelW[v] !== 0) return 0
    // Beside a real neighbour's own label the two would read as one text.
    const row = layered.layers[layerOf[v]]
    const prev = row[row.indexOf(v) - 1]
    return prev !== undefined && prev < graph.nodes.length && labelPad[prev] > 0 ? 0 : labelPad[v]
  })
  let [lo, box, text] = extentsOf(centers)
  single.forEach((w, v) => {
    if (w === 0) return
    labelPad[v] = 0
    labelPadLeft[v] = w
    const next = place()
    const [nextLo, nextBox, nextText] = extentsOf(next)
    // Boxes end sooner on the right (or a label past them does) without
    // hanging further out on the left: a label past the origin only
    // shifts the whole drawing.
    const narrower = nextBox < box || (nextBox === box && Math.max(nextBox, nextText) < Math.max(box, text))
    if (narrower && nextLo >= lo) {
      ;[lo, box, text] = [nextLo, nextBox, nextText]
      centers = next
      labelFlipped.add(v)
    } else {
      labelPad[v] = w
      labelPadLeft[v] = 0
    }
  })
  const boxL = (j: number): number => sat(centers[j], half(sizes.boxW[j]))
  const boxR = (j: number): number => boxL(j) + sizes.boxW[j] - 1
  const port = (node: number, side: number): number =>
    Math.max(boxL(node) + 1, Math.min(boxR(node) - 1, centers[node] + 2 * side))
  // An exterior return can clear every intermediate box yet still run
  // over its wider source. Move a straight, unshared outer chain just
  // beyond that source so the existing side-port checks can use it. Keep
  // straight top exits and interior/shared chains in their reserved slots.
  graph.edges.forEach((e, i) => {
    const chain = layered.chains[i]
    if (!isBack(e) || extras[e.from].kind !== 'plain' || chain.length === 0) return
    const col = centers[chain[0]]
    const side = Math.sign(col - centers[e.from])
    if (side === 0 || Math.abs(col - port(e.from, side)) <= 1) return
    const outer = side < 0 ? 0 : -1
    if (layered.layers[ranks[e.from]].at(outer) !== e.from) return
    if (chain.some((v) => centers[v] !== col || layered.shared.has(v) || layered.layers[layerOf[v]].at(outer) !== v)) return
    const left = centers[e.from] - half(sizes.boxW[e.from])
    const next = side < 0 ? left - 3 : left + sizes.boxW[e.from] + 2
    if ((next - col) * side <= 0) return
    const candidate = [...centers]
    for (const v of chain) candidate[v] = next
    const ports = sidePorts(candidate)
    if (ports.exit[i] !== side) return
    // Preserve an existing clear target-side entry.
    const entry = sidePorts(centers).entry[i]
    if (entry !== 0 && ports.entry[i] !== entry) return
    centers = candidate
  })
  // A return entering on the left labels leftward; give the leftmost such
  // label room before the first column.
  let margin = Math.max(0, -extentsOf(centers)[0])
  graph.edges.forEach((e, i) => {
    const text = edgeText(e)
    if (!isBack(e) || entrySide[i] >= 0 || text === null || chainLabel[i] !== null) return
    margin = Math.max(margin, labelCols(text, maxLabel) + 1 - port(e.to, -1))
  })
  for (const v of labelFlipped) margin = Math.max(margin, labelPadLeft[v] - centers[v])
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
  const isFwd = (e: Edge): boolean => e.from !== e.to && ranks[e.to] === ranks[e.from] + 1
  const edgeEntryX = new Array<number>(graph.edges.length).fill(-1)
  const edgeLabelLeft = new Array<boolean>(graph.edges.length).fill(false)
  const labelW = (i: number): number => {
    if (chainLabel[i] !== null) return -1
    const text = edgeText(graph.edges[i])
    return text === null ? -1 : labelCols(text, maxLabel)
  }
  const { entry: sideEntry, exit: sideExit } = sidePorts(centers)
  const into: number[][] = graph.nodes.map(() => [])
  graph.edges.forEach((e, i) => {
    if ((isSkip(e) && sideEntry[i] === 0) || isFwd(e)) into[e.to].push(i)
  })
  graph.nodes.forEach((_, t) => {
    const entries = into[t]
    if (entries.length === 0) return
    const cx = centers[t]
    const left = boxL(t)
    const right = boxR(t)
    const arrives = (i: number): number => centers[layered.chains[i].at(-1) ?? graph.edges[i].from]
    /** One entry: a single edge, or the forwards merged onto one arrival. */
    type Item = { slot: number; w: number; edges: number[]; key: number }
    const item = (edges: number[], key: number): Item => ({
      slot: 0,
      w: Math.max(...edges.map(labelW)),
      edges,
      key,
    })
    const fwds = entries.filter((i) => isFwd(graph.edges[i]))
    const over = (i: number): boolean => arrives(i) > left && arrives(i) < right
    const flanked = fwds.some((i) => arrives(i) <= left) && fwds.some((i) => arrives(i) >= right)
    // Forwards that read alike (no label, or the same label) concentrate:
    // when any of them jogs in, every one joins that arrival — one head for
    // the fan-in, on the centre when a straight drop is among them
    // (Newbery's edge concentration). One label then names them all.
    const plain = fwds.every((i) => edgeText(graph.edges[i]) === edgeText(graph.edges[fwds[0]]))
    const someJog = fwds.some((i) => !over(i) || flanked)
    const jogging = fwds.filter((i) => !over(i) || flanked || (plain && someJog))
    // An unlabelled skip arriving beside a jogging fan joins it: one head
    // for the fan-in rather than a second `▼` a cell over (edge
    // concentration, as dot's `concentrate`).
    // An unlabelled skip joins the fan-in too: the jogging fan when there
    // is one, else the straight drop on the centre (its last jog then ends
    // on the centre column).
    const centreDrop = fwds.find((i) => over(i) && Math.abs(arrives(i) - cx) <= 1 && !jogging.includes(i))
    const host = jogging.length > 0 ? jogging : centreDrop === undefined ? [] : [centreDrop]
    // Only edges drawn alike may share a head: a dotted association and a
    // solid inheritance arrow are two things.
    const alike = (i: number, k: number): boolean =>
      graph.edges[i].line === graph.edges[k].line &&
      graph.edges[i].headTo === graph.edges[k].headTo &&
      edgeText(graph.edges[i]) === edgeText(graph.edges[k])
    const joins = entries.filter((i) => isSkip(graph.edges[i]) && host.length > 0 && host.some((k) => alike(i, k)))
    const merged = [...host, ...joins]
    // Skips whose chains concentrated into one trunk arrive on one column
    // and read alike: one head, not one per edge forking a cell above.
    const loose = entries.filter((i) => !merged.includes(i))
    const byArrival: number[][] = []
    for (const i of loose) {
      const g = byArrival.find((group) => isSkip(graph.edges[i]) && isSkip(graph.edges[group[0]]) && arrives(group[0]) === arrives(i) && alike(i, group[0]))
      if (g === undefined) byArrival.push([i])
      else g.push(i)
    }
    let items: Item[] = byArrival.map((group) => item(group, arrives(group[0])))
    // One head per way of reading the merged arrivals: alike ones share
    // it, so no label is ever folded under another.
    const groups: number[][] = []
    for (const i of merged) {
      const g = groups.find((group) => alike(i, group[0]))
      if (g === undefined) groups.push([i])
      else g.push(i)
    }
    for (const group of groups) {
      const hosts = group.filter((i) => host.includes(i))
      const anchors = hosts.length > 0 ? hosts : group
      const drop = anchors.find((i) => over(i) && Math.abs(arrives(i) - cx) <= 1)
      items.push(item(group, drop === undefined ? anchors.reduce((a, i) => a + centers[graph.edges[i].from], 0) / anchors.length : cx))
    }
    // Slots: an arrival at most a cell off centre snaps to it (routeForward
    // straightens such a jog), other in-range arrivals keep their column,
    // the rest spread evenly over the top. Then walk left to right with a
    // cursor over the free head-row cells: each entry lands at its slot
    // (or past the previous label), its own label going right when the
    // next slot leaves room, else left when the cells behind the cursor
    // allow. Null when the top runs out of room.
    const walk = (list: Item[], packed = false): { cols: number[]; lefts: boolean[] } | null => {
      list.sort((a, b) => a.key - b.key || a.edges[0] - b.edges[0])
      const fixed = list.filter((it) => it.key > left && it.key < right)
      for (const item of fixed) item.slot = Math.abs(item.key - cx) <= 1 ? cx : item.key
      const spread = (group: Item[], lo: number, hi: number): void => {
        group.forEach((item, i) => {
          const at = packed ? left + 1 : lo + Math.round(((hi - lo) * (i + 1)) / (group.length + 1))
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
        // A flipped box's first label takes the room reserved left of the
        // centre, so its arrow sits no further left than that.
        const flipped = i === 0 && item.w >= 0 && labelFlipped.has(t)
        const x = Math.max(item.slot, cursor, flipped ? cx : 0)
        if (x > right - 1) return null
        const next = list[i + 1]?.slot ?? Number.MAX_SAFE_INTEGER
        const w = item.w
        // A label sits a cell off its arrow: `▼ yes`, `yes ▼`.
        if (w >= 0 && (flipped || (x + w + 3 > next && x - cursor >= w + 1))) {
          lefts.push(true)
          cursor = x + 2
        } else {
          lefts.push(false)
          cursor = w >= 0 ? x + w + 3 : x + 2
        }
        cols.push(x)
      }
      return { cols, lefts }
    }
    // Spread over the top when there is room; else packed from the left.
    let fit = walk(items) ?? walk(items, true)
    // No room for a head each: forwards that read alike merge into one
    // arrival on the centre and the skips spread around it.
    if (fit === null && fwds.length > jogging.length && fwds.every((i) => alike(i, fwds[0]))) {
      items = [...items.filter((it) => !fwds.includes(it.edges[0])), item(fwds, cx)]
      fit = walk(items)
    }
    if (fit !== null) {
      items.forEach((it, i) => {
        for (const ei of it.edges) {
          edgeEntryX[ei] = fit.cols[i]
          edgeLabelLeft[ei] = fit.lefts[i]
        }
      })
      return
    }
    // Legacy: the centre goes to a skip whose chain comes straight down
    // it, else to the forwards merged; every other entry lands two cells
    // off on the side it arrives from (past the centre's label), or the
    // other side with its label flipped left, or merges onto the centre.
    const straight = entries.find((i) => isSkip(graph.edges[i]) && arrives(i) === cx)
    const centred = straight === undefined ? fwds : [straight]
    const reach = centred.length > 0 ? Math.max(cx, ...centred.map((i) => cx + 1 + labelW(i))) : -1
    for (const si of entries) {
      if (centred.includes(si)) {
        edgeEntryX[si] = cx
        continue
      }
      const clear = reach === -1 ? cx + 2 : reach + 2
      const fromLeft = arrives(si) < cx
      const leftOk = cx - 2 >= left + 1
      const rightOk = clear <= right - 1
      if ((fromLeft && leftOk) || (!rightOk && leftOk)) {
        edgeEntryX[si] = cx - 2
        edgeLabelLeft[si] = true
      } else if (rightOk) edgeEntryX[si] = clear
      else edgeEntryX[si] = cx
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
    edgeExitX[i] = sideExit[i] !== 0 ? next : Math.abs(exit - next) <= 1 ? next : exit
  })
  // A side entry's chain ends on its own column; the route turns into the
  // box from there.
  graph.edges.forEach((e, i) => {
    if (sideEntry[i] !== 0) {
      edgeEntryX[i] = centers[layered.chains[i].at(-1) as number]
      edgeLabelLeft[i] = false
    }
  })
  const jogs = chainJogs(graph, ranks, layered, centers, (e, i) => {
    if (isSkip(e)) return { exit: centers[e.from], entry: edgeEntryX[i] }
    return isBack(e) ? { exit: edgeExitX[i], entry: edgeEntryX[i] } : null
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
    row.length === 0 ? 3 : Math.max(...row.map((i) => sizes.boxH[i])),
  )
  const rankY = new Array<number>(maxRank + 1).fill(0)
  for (let r = 1; r <= maxRank; r++) {
    rankY[r] = rankY[r - 1] + rankH[r - 1] + Math.max(GAP_Y, busTracks[r - 1] + 1)
  }
  const canvasH = rankY[maxRank] + rankH[maxRank]
  const bandEnd = Array.from({ length: maxRank + 1 }, (_, r) => rankY[r] + rankH[r])
  const jogRoute = skipRoutes(graph, jogs, (j) => bandEnd[j.band] + (jogTrack.get(j) ?? 0))

  let diagramW = 1
  for (let v = graph.nodes.length; v < centers.length; v++) diagramW = Math.max(diagramW, centers[v] + 1)
  byRank.forEach((row, r) => {
    for (const idx of row) {
      const w = sizes.boxW[idx]
      const h = sizes.boxH[idx]
      const cx = centers[idx]
      const x = sat(cx, half(w))
      const y = rankY[r] + half(rankH[r] - h)
      placed[idx] = { x, y, w, h, cx, cy: y + half(h), rank: r }
      diagramW = Math.max(diagramW, x + w)
      if (sizes.selfLabelW[idx] > 0) diagramW = Math.max(diagramW, x + w + 4 + sizes.selfLabelW[idx])
    }
  })

  const edgeLabelAt = chainLabel.map((label) => {
    if (label === null) return null
    const { v, w, side } = label
    const r = layerOf[v]
    return { row: rankY[r] + half(rankH[r]), x: side > 0 ? centers[v] + 2 : centers[v] - 1 - w }
  })
  let contentW = diagramW
  graph.edges.forEach((e, i) => {
    if (e.from === e.to) return
    const label = chainLabel[i]
    if (label !== null) {
      contentW = Math.max(contentW, (edgeLabelAt[i] as { x: number }).x + label.w)
    } else if (ranks[e.to] > ranks[e.from]) {
      const text = edgeText(e)
      if (text !== null) contentW = Math.max(contentW, Math.max(placed[e.to].cx, edgeEntryX[i]) + 2 + labelCols(text, maxLabel))
    } else {
      const text = edgeText(e)
      if (text !== null) {
        // routeBackChain starts the label right of the entry column.
        contentW = Math.max(contentW, edgeEntryX[i] + 2 + labelCols(text, maxLabel))
      }
    }
  })

  const routes = graph.edges.map((edge, i): Route => {
    const from = placed[edge.from]
    const to = placed[edge.to]
    if (edge.from === edge.to) return selfRoute(from, edge, maxLabel, hookSide[edge.from] < 0 ? 'left' : 'right')
    if (isBack(edge)) {
      return backChainRoute(from, to, edge, edgeExitX[i], edgeEntryX[i], jogRoute[i], edgeLabelLeft[i], edgeLabelAt[i], maxLabel, sideEntry[i], sideExit[i])
    }
    if (isSkip(edge)) {
      return chainRoute(from, to, edge, edgeEntryX[i], jogRoute[i], edgeLabelLeft[i], edgeLabelAt[i], maxLabel, sideEntry[i])
    }
    return forwardRoute(from, to, edge, bandEnd[from.rank] + edgeBus[i], edgeEntryX[i], edgeLabelLeft[i], maxLabel)
  })
  return { canvasW: contentW, canvasH, routes }
}

function placeLr(
  ranks: number[],
  maxRank: number,
  byRank: number[][],
  layered: Layered,
  sizes: NodeSizes,
  graph: Graph,
  placed: Placed[],
  extras: NodeExtra[],
): Plan {
  const colW = byRank.map((row) =>
    row.length === 0 ? 0 : Math.max(...row.map((i) => sizes.boxW[i])),
  )

  // Cross-frame ends port at the inner node: forward edges through the
  // frame's sides, laned ones through its bottom. Sides are tried first so
  // a skip can prove itself straight on its port row; the final pass then
  // resolves every end for real, with lane-bound ones on the bottom.
  type Ends = [Port | null, Port | null]
  // Edges leaving (or entering) one inner node through one side share its
  // port, so a fan draws one stem instead of a row of parallel ones.
  const resolve = (sides: (i: number) => [Side, Side] | null): Ends[] => {
    const used = graph.nodes.map(() => new Set<string>())
    const shared = new Map<string, Port>()
    return graph.edges.map((e, i): Ends => {
      const s = sides(i)
      if (s === null) return [null, null]
      const port = (n: number, a: Anchor | undefined, side: Side, out: boolean): Port | null => {
        const extra = extras[n]
        if (a === undefined || extra.kind !== 'frame') return null
        const key = `${n}:${side}:${a.node}:${out}`
        let p = shared.get(key)
        if (p === undefined) {
          p = framePort(extra.sub, sizes.boxW[n], sizes.boxH[n], sizes.titleW[n], a, side, used[n])
          // A port on the frame border names no node beyond its row, and
          // a column not even that (a rank stacks nodes on one column), so
          // edges wanting the same row share a side port and every edge
          // shares a top or bottom one.
          if (p.through.length === 0) {
            const vertical = side === 'top' || side === 'bottom'
            const fallbackKey = `${n}:${side}:${out}:${vertical ? '' : p.wanted}`
            const prior = shared.get(fallbackKey)
            if (prior !== undefined) {
              used[n].delete(`${side}:${vertical ? p.box.cx : p.box.cy}`)
              p = prior
            } else shared.set(fallbackKey, p)
          }
          shared.set(key, p)
        }
        return p
      }
      return [port(e.from, e.fromAnchor, s[0], true), port(e.to, e.toAnchor, s[1], false)]
    })
  }
  const forward = (e: Edge): boolean => e.from !== e.to && ranks[e.to] > ranks[e.from]
  let ends = resolve((i) => (forward(graph.edges[i]) ? ['right', 'left'] : null))
  // A node whose incoming edges all leave their frames at one row off the
  // frame's centre sits that far off its own aligned position, so the
  // edges run straight rather than jog to it (`[*]` after a composite
  // state, a frame beside a frame).
  const delta = (i: number, end: 0 | 1): number => {
    const p = ends[i][end]
    const node = end === 0 ? graph.edges[i].from : graph.edges[i].to
    return p === null ? 0 : p.box.cy - half(sizes.boxH[node])
  }
  const align = new Map<number, number>()
  graph.nodes.forEach((_, v) => {
    const wants = graph.edges.flatMap((e, i) =>
      e.to === v && e.from !== v && ranks[e.from] + 1 === ranks[v] ? [delta(i, 0) - delta(i, 1)] : [],
    )
    if (wants.length > 0 && wants[0] !== 0 && wants.every((w) => w === wants[0])) align.set(v, wants[0])
  })
  // A self-loop's stub hangs two rows below its box; room on both sides
  // keeps the box centred on its row.
  const loops = new Set(graph.edges.filter((e) => e.from === e.to).map((e) => e.from))
  const layH = sizes.layH.map((h, i) => (loops.has(i) ? h + 4 : h))
  const centers = assignPositions(layered, layH, 1, undefined, (v) => align.get(v) ?? 0)

  // A skip whose target entry row crosses no box on any intermediate rank
  // runs straight through the diagram into the target's left side, exiting
  // through the source's right-side fan; the bottom lane is the fallback.
  // (No entry spreading or local returns here: LR boxes are three rows tall,
  // so the centre row is the only usable port on a side.)
  const isSkip = (e: Edge): boolean => e.from !== e.to && ranks[e.to] - ranks[e.from] > 1
  const boxTop = (i: number): number => sat(centers[i], half(sizes.boxH[i]))
  // A back-edge target's top-entry `▼` stub sits one row above its box;
  // a straight run through that cell would appear to carry the arrival.
  const stubRows = new Set<number>()
  for (const e of graph.edges) {
    if (e.from === e.to || ranks[e.to] >= ranks[e.from]) continue
    stubRows.add(boxTop(e.to) - 1)
  }
  // A tall box (a two-line label) has a row per text line on its left
  // side; arrivals that read differently (a dotted beside a solid, or
  // differently labelled) each take one, so neither lands on the other's
  // head and loses its style. Same-reading arrivals still share the centre.
  // Only arrivals from one source spread: from two sources, two rows mean
  // two columns, and the inner one cuts the outer one's approach (see
  // proofs/PrivateFanIn.lean), so those share the centre and the head goes solid.
  const rowOffsets = (pick: (e: Edge, v: number) => boolean): Map<number, number> => {
    const out = new Map<number, number>()
    graph.nodes.forEach((_, v) => {
      const rows = sizes.boxH[v] - 2
      if (rows < 2) return
      const kinds: string[] = []
      const edges = graph.edges.flatMap((e, i) => (e.from !== e.to && pick(e, v) ? [i] : []))
      if (new Set(edges.map((i) => graph.edges[i].from)).size > 1) return
      // Plain solid edges keep the centre row; the odd one out moves.
      const read = (i: number): string => `${graph.edges[i].line}|${edgeText(graph.edges[i]) ?? ''}`
      for (const i of edges) if (!kinds.includes(read(i))) kinds.push(read(i))
      kinds.sort((a, b) => Number(a !== 'solid|') - Number(b !== 'solid|'))
      if (kinds.length < 2) return
      // Centre first, then the row above, then below.
      const slots = [0, -1, 1].filter((d) => d + half(sizes.boxH[v]) >= 1 && d + half(sizes.boxH[v]) <= rows)
      for (const i of edges) {
        const slot = kinds.indexOf(read(i))
        if (slot < slots.length) out.set(i, slots[slot])
      }
    })
    return out
  }
  const entryOffset = rowOffsets((e, v) => e.to === v && ranks[e.from] < ranks[v])
  const exitRow = (i: number): number => {
    const p = ends[i][0]
    return p === null ? centers[graph.edges[i].from] : boxTop(graph.edges[i].from) + p.box.cy
  }
  const entryRow = (i: number): number => {
    const p = ends[i][1]
    return p === null ? centers[graph.edges[i].to] + (entryOffset.get(i) ?? 0) : boxTop(graph.edges[i].to) + p.box.cy
  }
  // A skip whose target row crosses no box on any intermediate rank runs
  // straight through the diagram into the target's left side; otherwise
  // the bottom lane. (No chains here: LR back edges must lane, and a
  // diagram mixing interior skips with laned returns crosses itself.)
  const edgeStraight = new Array<boolean>(graph.edges.length).fill(false)
  const clearRow = (e: Edge, row: number): boolean =>
    !stubRows.has(row) &&
    graph.nodes.every(
      (_, j) =>
        ranks[j] <= ranks[e.from] ||
        ranks[j] >= ranks[e.to] ||
        Math.abs(centers[j] - row) > half(sizes.boxH[j]),
    )
  graph.edges.forEach((e, i) => {
    if (isSkip(e) && clearRow(e, entryRow(i))) edgeStraight[i] = true
  })
  ends = resolve((i) => {
    const e = graph.edges[i]
    if (e.from === e.to) return null
    if (ranks[e.to] === ranks[e.from] + 1 || edgeStraight[i]) return ['right', 'left']
    return ranks[e.to] < ranks[e.from] ? ['top', 'top'] : ['bottom', 'bottom']
  })
  // Every edge between two frames rides one bus: a trunk that fans out at
  // each end to the ports, instead of a column per edge.
  const biclique = bicliqueKeys(graph, ranks)
  const bundleOf = (i: number): string | undefined =>
    ends[i][0] !== null && ends[i][1] !== null ? `${graph.edges[i].from}>${graph.edges[i].to}` : biclique.get(i)
  const entryY = graph.edges.map((_, i) => (edgeStraight[i] ? entryRow(i) : -1))
  const jogs = chainJogs(graph, ranks, layered, centers, (e, i) =>
    entryY[i] === -1 ? null : { exit: exitRow(i), entry: entryY[i] },
  )
  const jogTrack = new Map<ChainJog, number>()

  // Left-to-right edge labels sit in the gap after their source's column, so
  // each gap sizes to the widest label *leaving through it* — one long label
  // widens its own band, not the whole diagram. Straight skips label there
  // too; a self-loop's label hangs beside its own box (selfLabelW).
  // (`bandLabel` is filled once the bus tracks are known: a label sits
  // right of its own bus, so it needs its track's offset as well.)
  const bandLabel = new Array<number>(maxRank + 1).fill(0)

  const edgeBus = new Array<number>(graph.edges.length).fill(0)
  const busTracks = new Array<number>(maxRank + 1).fill(0)
  for (let r = 0; r < maxRank; r++) {
    const spans = busSpans(graph, ranks, centers, r, true, entryRow, exitRow, bundleOf)
    const bandJogs = jogs.filter((j) => j.band === r)
    spans.push(...bandJogs)
    if (spans.length === 0) continue
    const { assigned, count } = assignTracks(spans)
    for (const [idx, slot] of assigned) edgeBus[idx] = slot
    for (const j of bandJogs) jogTrack.set(j, edgeBus[j.edge])
    // Tracks two columns apart, so parallel runs read as separate lines.
    busTracks[r] = count * 2 - 1
  }
  // A label belongs on the run this edge does not share. Departure is the
  // default; an edge leaving a source others leave too, into a target
  // nothing else enters, labels its arrival instead.
  // Only a label that lands in this band competes for the run: a laned
  // one is drawn along its lane, far from either end.
  const labelled = (k: number): boolean => {
    const o = graph.edges[k]
    return o.label !== null && o.from !== o.to && (ranks[o.to] === ranks[o.from] + 1 || edgeStraight[k])
  }
  const labelAtArrival = (i: number): boolean => {
    const e = graph.edges[i]
    if (e.label === null) return false
    const shares = (pick: (o: Edge) => number, at: number): boolean =>
      graph.edges.some((o, k) => k !== i && labelled(k) && pick(o) === at)
    return shares((o) => o.from, e.from) && !shares((o) => o.to, e.to)
  }
  graph.edges.forEach((e, i) => {
    if (e.from === e.to) return
    if (ranks[e.to] !== ranks[e.from] + 1 && !edgeStraight[i]) return
    // A label past the bus needs one column more, to end clear of the box.
    // Only a straight edge keeps its label before the bus.
    const past = exitRow(i) !== entryRow(i) || bundleOf(i) !== undefined
    const clearance = past ? busTracks[ranks[e.from]] : 2 * edgeBus[i]
    const verb = e.label === null ? 0 : labelCols(e.label, sizes.maxLabel) + clearance
    bandLabel[ranks[e.from]] = Math.max(bandLabel[ranks[e.from]], verb)
  })

  // A sink wider than the rest of its rank need not widen the column,
  // which would stretch every edge leaving its neighbours: with nothing
  // leaving it, it keeps the column's left edge and overhangs the band and
  // the next rank where its rows meet no box, bus or run. (Wide sinks are
  // the common case in class diagrams: a signature-heavy leaf.) The
  // overhang must end before the rank after next, so its extent is checked
  // once the columns are known and a sink that reaches too far rejoins its
  // column.
  const top = (i: number): number => sat(centers[i], half(sizes.boxH[i]))
  const rowsMeet = (i: number, lo: number, hi: number): boolean => top(i) <= hi && lo < top(i) + sizes.boxH[i]
  const goesAround = (i: number): boolean => {
    const e = graph.edges[i]
    return e.from !== e.to && ranks[e.to] !== ranks[e.from] + 1 && !edgeStraight[i]
  }
  const bandRows = (i: number, r: number): [number, number] | null => {
    const e = graph.edges[i]
    if (e.from === e.to || goesAround(i) || ranks[e.from] > r || ranks[e.to] <= r) return null
    const entry = entryRow(i)
    return ranks[e.from] === r ? [Math.min(exitRow(i), entry), Math.max(exitRow(i), entry)] : [entry, entry]
  }
  const overhangs = (i: number): boolean => {
    const r = ranks[i]
    if (r === maxRank || extras[i].kind === 'frame') return false
    if (graph.edges.some((e, k) => e.from === i || (e.to === i && goesAround(k)))) return false
    // A laned edge drops a vertical leg from its endpoint's box to the
    // lane outside the diagram. The leg's column is not known here, so a
    // sink beside either endpoint keeps its column rather than risk
    // standing on one (an edge behind a box reads worse than a crossing:
    // Ruegg et al., GD 2015).
    if (graph.edges.some((e, k) => goesAround(k) && [e.from, e.to].some((v) => ranks[v] === r || ranks[v] === r + 1)))
      return false
    const lo = top(i)
    const hi = lo + sizes.boxH[i] - 1
    const next = layered.layers[r + 1]
    if (next.some((v) => (v < graph.nodes.length ? rowsMeet(v, lo, hi) : lo <= centers[v] && centers[v] <= hi))) return false
    return graph.edges.every((_, k) => {
      const rows = bandRows(k, r)
      return rows === null || rows[1] < lo || hi < rows[0]
    })
  }
  const overhang = new Set(graph.nodes.flatMap((_, i) => (overhangs(i) ? [i] : [])))
  const rankX = new Array<number>(maxRank + 1).fill(0)
  for (;;) {
    byRank.forEach((row, r) => {
      const kept = row.filter((i) => !overhang.has(i))
      colW[r] = kept.length === 0 ? 0 : Math.max(...kept.map((i) => sizes.boxW[i]))
    })
    for (let r = 1; r <= maxRank; r++) {
      // Buses start one column clear of the rank's right edge and end one
      // clear of the arrowheads, so a frame border and a bus never read as a
      // double wall and a head never sits on a bus.
      const gap = Math.max(GAP_X + 1, bandLabel[r - 1] + 3, busTracks[r - 1] + 3)
      rankX[r] = rankX[r - 1] + colW[r - 1] + gap
    }
    const tooFar = [...overhang].filter(
      (i) =>
        sizes.boxW[i] <= colW[ranks[i]] ||
        (ranks[i] + 2 <= maxRank && rankX[ranks[i]] + sizes.boxW[i] >= rankX[ranks[i] + 2]),
    )
    if (tooFar.length === 0) break
    for (const i of tooFar) overhang.delete(i)
  }
  const selfTails = byRank[maxRank].filter((i) => sizes.selfLabelW[i] > 0).map((i) => 4 + sizes.selfLabelW[i])
  const canvasW = Math.max(
    rankX[maxRank] + colW[maxRank] + (selfTails.length === 0 ? 0 : Math.max(...selfTails)),
    ...[...overhang].map((i) => rankX[ranks[i]] + sizes.boxW[i]),
  )
  const bandEnd = Array.from({ length: maxRank + 1 }, (_, r) => rankX[r] + colW[r])
  const skipRoute = skipRoutes(graph, jogs, (j) => bandEnd[j.band] + 1 + (jogTrack.get(j) ?? 0))

  let diagramH = 1
  for (let v = graph.nodes.length; v < centers.length; v++) diagramH = Math.max(diagramH, centers[v] + 1)
  // A box sits at the column edge its own edges use: the right edge when
  // something leaves it, so a narrow box beside a wide one reaches the
  // next rank in a short run rather than one as wide as the widest label;
  // the left edge when nothing leaves, which shortens what arrives
  // instead. A sink wide enough to overhang keeps the left edge too.
  const sink = graph.nodes.map((_, i) => !graph.edges.some((e) => e.from === i && e.to !== i))
  byRank.forEach((row, r) => {
    for (const idx of row) {
      const w = sizes.boxW[idx]
      const h = sizes.boxH[idx]
      const cy = centers[idx]
      const y = sat(cy, half(h))
      const x = overhang.has(idx) || sink[idx] ? rankX[r] : rankX[r] + colW[r] - w
      placed[idx] = { x, y, w, h, cx: x + half(w), cy: y + half(h), rank: r }
      diagramH = Math.max(diagramH, y + h + (loops.has(idx) ? 2 : 0))
    }
  })

  const endsOf = (i: number): [Placed, Placed] => {
    const e = graph.edges[i]
    const [pf, pt] = ends[i]
    return [
      pf === null ? placed[e.from] : portAt(pf, placed[e.from]),
      pt === null ? placed[e.to] : portAt(pt, placed[e.to]),
    ]
  }
  // Returns run in lanes above the diagram, skips below, so a reciprocal
  // pair never stacks and a lane's side says its direction — unless a box
  // stacked in the same rank sits between an endpoint and that side, in
  // which case the lane takes the other side rather than pierce it.
  // Above-lanes push everything down once their count is known (`topH`).
  const edgeLane = new Array<number>(graph.edges.length).fill(0)
  const lanes = laneSpans(graph, ranks, endsOf).filter((s) => !edgeStraight[s.edge])
  const isBack = (i: number): boolean => ranks[graph.edges[i].to] < ranks[graph.edges[i].from]
  // An overhanging sink from the rank before reaching past this column's
  // centre stands in the way like a box of the same rank.
  const clear = (j: number, up: boolean): boolean => {
    const beside = (k: number): boolean => (up ? placed[k].cy > placed[j].cy : placed[k].cy < placed[j].cy)
    return (
      byRank[ranks[j]].every((k) => k === j || beside(k)) &&
      [...overhang].every((k) => ranks[k] !== ranks[j] - 1 || placed[k].x + placed[k].w <= placed[j].cx || beside(k))
    )
  }
  const onTop = (i: number): boolean => {
    const { from, to } = graph.edges[i]
    const [up, down] = [clear(from, true) && clear(to, true), clear(from, false) && clear(to, false)]
    return isBack(i) ? up || !down : !down && up
  }
  // A lane whose ends the drawing already connects the long way takes the
  // outermost track: it reaches around the informative ones, never the
  // other way (`transitiveEdges`).
  const weak = transitiveEdges(graph)
  const above = packTracks(lanes.filter((s) => onTop(s.edge)), weak)
  const below = packTracks(lanes.filter((s) => !onTop(s.edge)), weak)
  const topH = above.count === 0 ? 0 : above.count + 1
  for (const [idx, slot] of above.assigned) edgeLane[idx] = above.count - 1 - slot
  for (const [idx, slot] of below.assigned) edgeLane[idx] = slot
  const canvasH = topH + diagramH + (below.count === 0 ? 0 : 1 + below.count)
  const laneBase = topH + diagramH + 1
  for (const p of placed) {
    p.y += topH
    p.cy += topH
  }
  for (const js of skipRoute) for (const j of js) j.at += topH

  // A lane enters its target on the centre column — two cells off, toward
  // its source, when a lane also leaves that box on the same side, so an
  // arriving line never runs up the column the departing ones run down.
  const laned = new Set(lanes.map((s) => s.edge))
  const laneEntry = (i: number, from: Placed, to: Placed): number => {
    const shared = graph.edges.some((o, k) => k !== i && o.from === graph.edges[i].to && laned.has(k) && onTop(k) === onTop(i))
    const want = shared
      ? Math.max(to.x + 1, Math.min(to.x + to.w - 2, to.cx + (from.cx < to.cx ? -2 : 2)))
      : to.cx
    // The leg climbs from the lane to this column, so it must miss every
    // box stacked between the two: entering on the centre of a box that
    // sits above another draws the line straight through it.
    const blocked = (col: number): boolean =>
      placed.some(
        (b, k) =>
          k !== graph.edges[i].to &&
          k < graph.nodes.length &&
          // One cell of clearance: a leg hugging a box lands on the
          // arrowhead of whatever arrives there.
          col >= b.x - 1 &&
          col <= b.x + b.w &&
          (onTop(i) ? b.y + b.h <= to.y : b.y >= to.y + to.h),
      )
    if (!blocked(want)) return want
    for (let d = 1; d < to.w; d++) {
      for (const col of [want - d, want + d]) {
        if (col > to.x && col < to.x + to.w - 1 && !blocked(col)) return col
      }
    }
    return want
  }
  const routes = graph.edges.map((edge, i): Route => {
    const max = sizes.maxLabel
    if (edge.from === edge.to) return selfRoute(placed[edge.from], edge, max, 'below')
    const [from, to] = endsOf(i)
    const through = ends[i].flatMap((p, k) => (p === null ? [] : portAt(p, placed[k === 0 ? edge.from : edge.to]).through))
    const route =
      to.rank === from.rank + 1
        ? forwardRouteLr(
            from,
            to,
            edge,
            bandEnd[from.rank] + 1 + 2 * edgeBus[i],
            max,
            bundleOf(i) !== undefined,
            to.cy + (entryOffset.get(i) ?? 0),
            labelAtArrival(i),
            [bandEnd[from.rank] + 1, bandEnd[from.rank] + 1 + Math.max(0, busTracks[from.rank] - 1)],
          )
        : to.rank > from.rank && edgeStraight[i]
          ? skipRouteLr(from, to, edge, skipRoute[i], max)
          : laneRoute(from, to, edge, onTop(i) ? edgeLane[i] : laneBase + edgeLane[i], max, onTop(i), laneEntry(i, from, to))
    return through.length === 0 ? route : { ...route, through: [...(route.through ?? []), ...through] }
  })
  return { canvasW, canvasH, routes }
}

// -------------------------------------------------------------------- canvas
/**
 * The geometry of a laid-out graph: canvas size, a box per node, a route
 * per edge (null for a self loop) and each node's wrapped label lines.
 * Pure data — `paint` in paint.ts turns it into a canvas, and tests or
 * metrics can read it without one.
 */
export interface Layout {
  w: number
  h: number
  placed: Placed[]
  routes: Route[]
  labels: string[][]
}

/** Rank, order, place and route a graph. Null when it is empty or over the cell cap. */
export function layout(graph: Graph, extras: NodeExtra[], limits: Limits): Layout | null {
  const n = graph.nodes.length
  if (n === 0) return null

  // Cardinalities read as one text with the verb, source end first:
  // `1 places *`. One label beside the line, not three rows of them.
  for (const e of graph.edges) {
    if (e.cardFrom === undefined && e.cardTo === undefined) continue
    e.label = edgeText(e)
    e.cardFrom = undefined
    e.cardTo = undefined
  }

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
  // so LR back edges go around in a lane below. Their endpoints go last
  // within the rank, or whatever the ordering put beyond them would sit in
  // that corridor and be cut through. A forward skip is ordered freely: it
  // runs straight through the interior when its row is clear (which the
  // ordering can only make likelier), and lanes only as a fallback, where
  // `onTop` picks the side no box blocks.
  const vertical = graph.dir === 'down' || graph.dir === 'up'
  const interior = (): boolean => vertical
  const inLane = new Array<boolean>(graph.nodes.length).fill(false)
  for (const e of graph.edges) {
    if (e.from !== e.to && ranks[e.to] < ranks[e.from] && !vertical) {
      inLane[e.from] = true
      inLane[e.to] = true
    }
  }
  const layered = orderRanks(byRank, graph.edges, ranks, interior, inLane)

  const wrapped = graph.nodes.map((node) => wrapLabel(node.label, limits.wrap, limits.lines))
  const widest = (lines: string[]): number =>
    Math.max(1, lines.length === 0 ? 1 : Math.max(...lines.map(stringWidth)))

  // Left-to-right returns port through a frame's top border, beside the
  // title: leave a column per return touching the frame.
  const topPorts = graph.nodes.map((_, i) =>
    vertical
      ? 0
      : graph.edges.filter((e) => ranks[e.to] < ranks[e.from] && (e.from === i || e.to === i)).length,
  )
  const boxW = extras.map((extra, i) => {
    if (extra.kind === 'frame') {
      // Fitted here for good: paint would otherwise stretch the title back
      // over the columns reserved beside it.
      const reserve = topPorts[i] > 0 ? topPorts[i] + 1 : 0
      if (reserve > 0) {
        graph.nodes[i].label = fitLabel(graph.nodes[i].label, Math.max(limits.wrap, extra.sub.w - reserve))
      }
      // A blank column inside each side keeps inner boxes off the border.
      return Math.max(extra.sub.w + 4, stringWidth(graph.nodes[i].label) + 4 + reserve)
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

  // A self-edge hooks the box's bottom-right corner; room beside the box
  // for the hook and its label.
  const selfLabelW = new Array<number>(n).fill(0)
  for (const e of graph.edges) {
    if (e.from !== e.to) continue
    boxW[e.from] = Math.max(boxW[e.from], 7)
    const text = edgeText(e)
    if (text !== null) {
      selfLabelW[e.from] = Math.max(selfLabelW[e.from], labelCols(text, limits.label))
    }
  }

  // Top-down, every forward arrival that reads differently gets its own
  // head on the target's top, each with its label beside it: the box is at
  // least that wide, or the walk would have to fold two texts onto one
  // head and lose one.
  if (vertical) {
    const reads = (e: Edge): string => `${e.line}|${e.headTo}|${edgeText(e)}`
    graph.nodes.forEach((_, j) => {
      const texts = new Set<string>()
      let need = 1
      graph.edges.forEach((e, i) => {
        if (e.to !== j || e.from === j || ranks[e.from] >= ranks[j] || texts.has(reads(e))) return
        // A parallel edge rides its first twin's cells.
        if (graph.edges.some((o, k) => k < i && o.from === e.from && o.to === e.to)) return
        texts.add(reads(e))
        // A skip's label rides beside its chain when there is room.
        const text = ranks[j] - ranks[e.from] > 1 ? null : edgeText(e)
        need += 2 + (text === null ? 0 : labelCols(text, limits.label) + 1)
      })
      // And wide enough for a straight drop on the centre with the other
      // heads two cells apart on one side, so the drop never bends to
      // make room (labels then spill past the box).
      if (texts.size > 1) boxW[j] = Math.max(boxW[j], need + 1, 4 * texts.size - 1)
    })
  }

  const sizes: NodeSizes = {
    boxW,
    boxH,
    // Top-down, the hook's side reserves its label through the pads.
    layW: boxW.map((w, i) => w + (selfLabelW[i] > 0 && !vertical ? 2 * (selfLabelW[i] + 4) : 0)),
    layH: boxH,
    selfLabelW,
    titleW: extras.map((extra, i) =>
      extra.kind === 'frame' && graph.nodes[i].label !== ''
        ? stringWidth(fitLabel(graph.nodes[i].label, sat(boxW[i], 4))) + 2
        : 0,
    ),
    maxLabel: limits.label,
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
    ? placeTd(ranks, maxRank, byRank, layered, sizes, graph, placed, extras)
    : placeLr(ranks, maxRank, byRank, layered, sizes, graph, placed, extras)

  if (plan.canvasW * plan.canvasH > MAX_CANVAS_CELLS) return null
  return { w: plan.canvasW, h: plan.canvasH, placed, routes: plan.routes, labels: wrapped }
}

type Jog = { bus: number; at: number }
type LabelAt = { row: number; x: number } | null

/** Route labels with their text fitted to `max` columns, as painted. */
const fitted = (labels: Route['labels'], max: number): Route['labels'] =>
  labels.map((l) => ({ ...l, text: fitLabel(l.text, max) }))

/**
 * A self loop: no corners — paint draws the stub below the box — and its
 * label beside the box on the loop's first row.
 */
function selfRoute(p: Placed, edge: Edge, max: number, hook: Hook): Route {
  const text = edgeText(edge)
  if (text === null) return { points: [], labels: [], hook }
  const label = fitLabel(text, max)
  const at =
    hook === 'right'
      ? { row: p.cy, x: p.x + p.w + 4 }
      : hook === 'left'
        ? { row: p.cy, x: p.x - 4 - stringWidth(label) }
        : { row: p.y + p.h, x: p.x + p.w + 1 }
  return { points: [], labels: [{ text: label, ...at }], hook }
}

/**
 * Corners of a path that follows its chain's jogs from `start`: each jog
 * runs along the flow axis to its bus, then across it to where the chain
 * continues.
 */
function jogPoints(start: [number, number], jogs: Jog[], vertical: boolean): [number, number][] {
  const points: [number, number][] = [start]
  let [x, y] = start
  for (const { bus, at } of jogs) {
    if (vertical) points.push([x, bus], [at, bus])
    else points.push([bus, y], [bus, at])
    ;[x, y] = vertical ? [at, bus] : [bus, at]
  }
  return points
}

/**
 * Adjacent ranks, top-down: out the source's bottom, jog on a bus row,
 * into the target's top. A jog of one column reads as a kink and snaps
 * straight. The label sits beside the head.
 */
function forwardRoute(
  from: Placed,
  to: Placed,
  edge: Edge,
  bus: number,
  entryX: number,
  labelLeft: boolean,
  max: number,
): Route {
  const tx = entryX === -1 ? to.cx : entryX
  const bx = Math.abs(from.cx - tx) <= 1 ? tx : from.cx
  const by = from.y + from.h - 1
  const headRow = to.y - 1
  const points: [number, number][] =
    bx === tx
      ? [
          [bx, by],
          [tx, headRow],
        ]
      : [
          [bx, by],
          [bx, bus],
          [tx, bus],
          [tx, headRow],
        ]
  const labels: Route['labels'] = []
  if (edge.label !== null) labels.push({ text: edge.label, row: headRow, x: labelStart(tx, edge.label, labelLeft, max) })
  return { points, labels: fitted(labels, max) }
}

/** A chain-routed edge's label: beside its chain when it has a spot there, else at the head. */
function chainLabel(
  edge: Edge,
  headRow: number,
  entryX: number,
  labelLeft: boolean,
  labelAt: LabelAt,
  max: number,
): Route['labels'] {
  const text = edgeText(edge)
  if (text === null) return []
  const at = labelAt ?? { row: headRow, x: labelStart(entryX, text, labelLeft, max) }
  return [{ text: fitLabel(text, max), ...at }]
}

/**
 * Back edge, top-down: up out of the source's top, along the column its
 * virtual chain reserved (jogging on a bus row wherever it steps), arrow
 * into the target's bottom. Adjacent returns have no chain and jog once.
 */
function backChainRoute(
  from: Placed,
  to: Placed,
  edge: Edge,
  exitX: number,
  entryX: number,
  jogs: Jog[],
  labelLeft: boolean,
  labelAt: LabelAt,
  max: number,
  side = 0,
  exitSide = 0,
): Route {
  // Out the side on the centre row to the chain column, or out the top port.
  const points =
    exitSide === 0
      ? jogPoints([exitX, from.y], jogs, true)
      : [[exitSide < 0 ? from.x - 1 : from.x + from.w, from.cy] as [number, number], ...jogPoints([exitX, from.cy], jogs, true)]
  if (side !== 0) return sideLeg(points, to, edge, entryX, side, labelAt, max)
  const headRow = to.y + to.h
  points.push([entryX, headRow])
  return { points, labels: chainLabel(edge, headRow, entryX, labelLeft, labelAt, max) }
}

/**
 * Finish a chain route through the target's side: along the chain column
 * to the centre row, then across into the box. A label beside the chain
 * stays there; otherwise it interrupts the leg, the way a lane label does.
 */
function sideLeg(points: [number, number][], to: Placed, edge: Edge, entryX: number, side: number, labelAt: LabelAt, max: number): Route {
  const head = side < 0 ? to.x - 1 : to.x + to.w
  points.push([entryX, to.cy], [head, to.cy])
  const text = edgeText(edge)
  if (labelAt !== null) return { points, labels: fitted([{ text: text ?? '', row: labelAt.row, x: labelAt.x }], max) }
  const laneLabel =
    text === null ? undefined : { text: ` ${fitLabel(text, max)} `, y: to.cy, lo: Math.min(entryX, head), hi: Math.max(entryX, head) }
  return { points, labels: [], laneLabel }
}

/**
 * Forward skip edge, top-down: out the source's *bottom*, then down the
 * column its virtual chain reserved, jogging along a bus row wherever the
 * chain steps sideways (the first jog shares the source's fan row — one `┴`
 * origin split; the last lands on the entry column) into the target's *top*.
 */
function chainRoute(
  from: Placed,
  to: Placed,
  edge: Edge,
  entryX: number,
  jogs: Jog[],
  labelLeft: boolean,
  labelAt: LabelAt,
  max: number,
  side = 0,
): Route {
  const points = jogPoints([from.cx, from.y + from.h - 1], jogs, true)
  if (side !== 0) return sideLeg(points, to, edge, entryX, side, labelAt, max)
  const headRow = to.y - 1
  points.push([entryX, headRow])
  return { points, labels: chainLabel(edge, headRow, entryX, labelLeft, labelAt, max) }
}

/**
 * Adjacent ranks, left-to-right: out the right side, jog on the bus
 * column. The verb keeps its usual spot above the line; cardinalities hug
 * their own ends on the rows above the departure and arrival cells.
 */
function forwardRouteLr(
  from: Placed,
  to: Placed,
  edge: Edge,
  bus: number,
  max: number,
  bundled = false,
  entry = to.cy,
  atArrival = false,
  band: [number, number] = [bus, bus],
): Route {
  const rx = from.x + from.w - 1
  const ry = from.cy
  const ly = entry
  const headCol = to.x - 1
  const points: [number, number][] =
    ry === ly && !bundled
      ? [
          [rx, ry],
          [headCol, ly],
        ]
      : [
          [rx, ry],
          [bus, ry],
          [bus, ly],
          [headCol, ly],
        ]
  const labels: Route['labels'] = []
  // The label goes on whichever of the edge's two runs it has to itself.
  // Edges into one target share the bus column and entry row; edges out of
  // one source share the departure row. Sitting on the shared one stacks
  // this label on the next edge's.
  //
  // Either way it clears the bus: the stretch between the source and the
  // bus is often too narrow to hold a word, and a label written there
  // lands on the edge's own trunk.
  if (edge.label !== null) {
    // A straight edge has no bus to clear: the whole run is its own.
    // Otherwise the label clears every track in the band, not just this
    // edge's: the neighbouring trunks run through the same rows.
    const straight = ry === ly && !bundled
    const fits = straight || rx + 2 + labelCols(edge.label, max) < band[0]
    const x = !straight && (atArrival || !fits) ? band[1] + 2 : rx + 2
    labels.push({ text: edge.label, row: sat(atArrival ? ly : ry, 1), x })
  }
  const route: Route = { points, labels: fitted(labels, max) }
  // A bundled edge meets the shared bus where it joins and leaves it.
  if (bundled) route.through = [[bus, ry, 'j'], [bus, ly, 'j']]
  return route
}

/**
 * Forward skip, left-to-right: out the source's right side, along the row
 * its virtual chain reserved, jogging on a bus column wherever the chain
 * steps (the first jog shares the source's fan column), into the target's
 * left side on its centre row. Label after the first jog, where forward
 * labels sit — the gap before the target belongs to the arrivals that end
 * there.
 */
function skipRouteLr(from: Placed, to: Placed, edge: Edge, jogs: Jog[], max: number): Route {
  const rx = from.x + from.w - 1
  const ry = from.cy
  const points = jogPoints([rx, ry], jogs, false)
  points.push([to.x - 1, to.cy])
  const text = edgeText(edge)
  const labels: Route['labels'] = []
  if (text !== null) labels.push({ text, row: sat(jogs[0]?.at ?? to.cy, 1), x: (jogs[0]?.bus ?? rx) + 1 })
  return { points, labels: fitted(labels, max) }
}

/**
 * Skip or back edge, left-to-right: down out the bottom, along a lane,
 * back up. The label interrupts its own lane row — the row above belongs
 * to the neighbouring lane once several stack — and waits until every
 * route landed so it can dodge the verticals that cross this row.
 */
function laneRoute(from: Placed, to: Placed, edge: Edge, laneY: number, max: number, top: boolean, tx: number): Route {
  const sx = from.cx
  const sy = top ? from.y : from.y + from.h - 1
  const points: [number, number][] = [
    [sx, sy],
    [sx, laneY],
    [tx, laneY],
    [tx, top ? to.y - 1 : to.y + to.h],
  ]
  const text = edgeText(edge)
  const laneLabel =
    text === null
      ? undefined
      : { text: ` ${fitLabel(text, max)} `, y: laneY, lo: Math.min(sx, tx), hi: Math.max(sx, tx) }
  return { points, labels: [], laneLabel }
}
