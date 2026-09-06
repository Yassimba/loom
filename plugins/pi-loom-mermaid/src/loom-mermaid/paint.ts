/**
 * Painting a `Layout` onto a `Canvas`: boxes, frames, routes, labels. The
 * only module that both knows the layout geometry and draws glyphs.
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
import type { Edge, Graph, Head, Shape } from './graph.ts'
import { fitLabel, MAX_LABEL } from './labels.ts'
import {
  edgeText,
  half,
  type LaneLabel,
  type Layout,
  type NodeExtra,
  PAD,
  type Placed,
  type Route,
  sat,
} from './layout.ts'
import { measured, stringWidth } from './width.ts'

/**
 * Paint a layout onto a fresh canvas: boxes (plain, class compartments or
 * subgraph frames), then every route, then the labels across them.
 */
export function paint(graph: Graph, extras: NodeExtra[], lay: Layout): Canvas {
  const { placed, routes } = lay
  const canvas = new Canvas(lay.w, lay.h)
  for (let idx = 0; idx < graph.nodes.length; idx++) {
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
    else drawBox(canvas, placed[idx], lay.labels[idx], graph.nodes[idx].shape, mirrored)
  }
  canvas.curTag = undefined
  canvas.curHref = undefined

  graph.edges.forEach((edge, i) => {
    canvas.curStyle =
      edge.line === 'dotted' ? STY_DOT : edge.line === 'thick' ? STY_THICK : STY_SOLID
    const route = routes[i]
    if (route === null) routeSelf(canvas, placed[edge.from], edge)
    else drawRoute(canvas, edge, route)
  })
  flushLabels(canvas)
  placeLaneLabels(canvas, routes.flatMap((r) => (r?.laneLabel === undefined ? [] : [r.laneLabel])))

  canvas.finalizeMask()
  return canvas
}

/** Apply the direction flip a finished canvas needs for `BT` / `RL`. */
export function orient(canvas: Canvas, graph: Graph): Canvas {
  if (graph.dir === 'up') canvas.flipVertical()
  else if (graph.dir === 'left') canvas.flipHorizontal()
  return canvas
}


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

/** Direction bit from one cell toward the next (they share a row or column). */
const toward = ([x0, y0]: [number, number], [x1, y1]: [number, number]): number =>
  x1 > x0 ? R : x1 < x0 ? L : y1 > y0 ? D : U

const ARROW: Record<number, string> = { [D]: '▼', [U]: '▲', [R]: '▶', [L]: '◄' }

/**
 * Paint a route: junction bits where it leaves the source border, a
 * segment per pair of corners, then the tail and head glyphs facing the
 * way the line leaves and arrives. A head of `none` just meets the box.
 */
function drawRoute(canvas: Canvas, edge: Edge, route: Route): void {
  const { points } = route
  const [sx, sy] = points[0]
  const [hx, hy] = points[points.length - 1]
  const leave = toward(points[0], points[1])
  const arrive = toward(points[points.length - 2], points[points.length - 1])
  canvas.junction(sx, sy, leave)
  for (let k = 0; k + 1 < points.length; k++) {
    const [x0, y0] = points[k]
    const [x1, y1] = points[k + 1]
    if (x0 === x1) canvas.segV(x0, y0, y1)
    else canvas.segH(y0, x0, x1)
  }
  if (edge.headTo === 'none') canvas.addBits(hx, hy, toward(points[points.length - 1], points[points.length - 2]))
  else canvas.set(hx, hy, headGlyph(edge.headTo, ARROW[arrive]), 'edge')
  if (edge.headFrom !== 'none') {
    canvas.set(sx, sy, headGlyph(edge.headFrom, ARROW[toward(points[1], points[0])]), 'edge')
  }
  for (const { text, row, x } of route.labels) placeLabel(canvas, text, row, x)
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
/** Queue a label to be written after every line, so it interrupts them. */
function placeLabel(canvas: Canvas, label: string, row: number, x: number): void {
  canvas.labels.push({ label, row, x })
}

function flushLabels(canvas: Canvas): void {
  for (const { label, row, x } of canvas.labels.splice(0)) writeLabel(canvas, label, row, x)
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
