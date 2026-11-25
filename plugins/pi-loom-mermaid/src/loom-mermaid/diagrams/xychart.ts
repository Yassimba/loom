/**
 * `xychart-beta`. Bars alone are horizontal, a row per category, on one
 * shared value scale. A chart with a `line` series is a plot: values up the
 * left axis, a column per category, bars as blocks and each line as braille
 * dots joined point to point. Lenient: an unreadable statement is dropped
 * and recorded in `warnings`.
 */

import type { Diagram } from '../registry.ts'
import { headerKind, nonEmpty, statementsOf, words } from '../statements.ts'
import { Canvas, drawText } from '../canvas.ts'
import { stringWidth } from '../width.ts'
import { afterKeyword, barOf, drawRows, type ListRow } from './rows.ts'

const BAR_W = 30
const FILLS = ['█', '▓', '▒']

const PLOT_H = 10
/** Braille dot bits by sub-cell: two columns of four rows. */
const DOTS = [
  [0x01, 0x02, 0x04, 0x40],
  [0x08, 0x10, 0x20, 0x80],
]

interface Series {
  name: string
  values: number[]
}

/** The plot a chart with a line series draws: axis, bars, lines, labels. */
function drawPlot(title: string | null, labels: string[], series: Series[], lo: number, hi: number): Canvas {
  const ticks = [hi, (lo + hi) / 2, lo].map((v) => String(Math.round(v * 100) / 100))
  const axisX = Math.max(...ticks.map(stringWidth)) + 1
  const colW = Math.max(6, ...labels.map((l) => stringWidth(l) + 2))
  const plotW = colW * labels.length
  const legend = series.map((s) => `${s.name === 'line' ? '⠒' : '█'} ${s.name}`).join('  ')
  const top = title === null ? 0 : 1
  const width = Math.max(axisX + 1 + plotW, stringWidth(title ?? ''), stringWidth(legend))
  const canvas = new Canvas(width, top + PLOT_H + 3)
  if (title !== null) drawText(canvas, title, Math.floor((width - stringWidth(title)) / 2), 0, 'title')
  // Height above the axis in quarter rows, the braille grid's resolution.
  const level = (v: number): number => Math.round(((Math.min(hi, Math.max(lo, v)) - lo) / Math.max(hi - lo, Number.EPSILON)) * (PLOT_H * 4 - 1))
  for (let y = 0; y < PLOT_H; y++) drawText(canvas, '│', axisX, top + y, 'border')
  ticks.forEach((tick, k) => {
    const y = top + Math.round((k * (PLOT_H - 1)) / 2)
    drawText(canvas, tick, axisX - 1 - stringWidth(tick), y, 'edgeLabel')
    drawText(canvas, '┤', axisX, y, 'border')
  })
  drawText(canvas, `└${'─'.repeat(plotW)}`, axisX, top + PLOT_H, 'border')
  labels.forEach((label, c) => {
    drawText(canvas, label, axisX + 1 + c * colW + Math.floor((colW - stringWidth(label)) / 2), top + PLOT_H + 1, 'text')
  })
  drawText(canvas, legend, 0, top + PLOT_H + 2, 'title')

  const centre = (c: number): number => axisX + 1 + c * colW + Math.floor(colW / 2)
  for (const s of series.filter((s) => s.name === 'bar')) {
    s.values.forEach((v, c) => {
      const rows = Math.max(1, Math.round((level(v) + 1) / 4))
      for (let k = 0; k < rows; k++) drawText(canvas, '██', centre(c) - 1, top + PLOT_H - 1 - k, 'edge')
    })
  }
  // Lines go over the bars: dots on a 2-by-4 grid inside each cell.
  const dots = new Map<number, number>()
  const dot = (px: number, py: number): void => {
    const [x, y] = [px >> 1, top + PLOT_H - 1 - (py >> 2)]
    dots.set(y * width + x, (dots.get(y * width + x) ?? 0) | DOTS[px & 1][3 - (py & 3)])
  }
  for (const s of series.filter((s) => s.name === 'line')) {
    s.values.slice(1).forEach((v, c) => {
      const [x0, y0, x1, y1] = [centre(c) * 2, level(s.values[c]), centre(c + 1) * 2, level(v)]
      for (let x = x0; x <= x1; x++) dot(x, Math.round(y0 + ((y1 - y0) * (x - x0)) / (x1 - x0)))
    })
    if (s.values.length === 1) dot(centre(0) * 2, level(s.values[0]))
  }
  for (const [i, bits] of dots) drawText(canvas, String.fromCharCode(0x2800 + bits), i % width, Math.floor(i / width), 'edge')
  return canvas
}

/** The `[a, b, c]` list in a statement, quotes peeled, or null. */
function listOf(st: string): string[] | null {
  const match = /\[(.*)\]/.exec(st)
  return match === null ? null : match[1].split(',').map((item) => item.trim().replace(/^"(.*)"$/, '$1'))
}

export const xychart: Diagram = {
  kind: 'xychart',
  headers: ['xychart-beta', 'xychart'],
  render(src) {
    const statements = statementsOf(src)
    const header = headerKind(statements)
    if (header === null || !xychart.headers.includes(header)) return null
    let title: string | null = null
    let categories: string[] | null = null
    let range: [number, number] | null = null
    const series: Series[] = []
    const warnings: string[] = []

    for (const st of statements.slice(1)) {
      const first = words(st)[0]?.toLowerCase()
      if (first === 'title') title = nonEmpty(afterKeyword(st))
      else if (first === 'x-axis') categories = listOf(st) ?? categories
      else if (first === 'y-axis') {
        const bounds = /(-?\d+(?:\.\d+)?)\s*-->\s*(-?\d+(?:\.\d+)?)/.exec(st)
        if (bounds !== null) range = [Number(bounds[1]), Number(bounds[2])]
      } else if (first === 'bar' || first === 'line') {
        const values = listOf(st)?.map(Number)
        if (values === undefined || values.some((v) => !Number.isFinite(v))) warnings.push(`dropped, unreadable statement: "${st}"`)
        else series.push({ name: first, values })
      }
    }
    if (series.length === 0) return null

    const all = series.flatMap((s) => s.values)
    const [lo, hi] = range ?? [Math.min(0, ...all), Math.max(...all)]
    const scale = (v: number): number => Math.round(((Math.min(hi, Math.max(lo, v)) - lo) / Math.max(hi - lo, Number.EPSILON)) * BAR_W)
    const count = Math.max(...series.map((s) => s.values.length))
    if (series.some((s) => s.name === 'line')) {
      const labels = Array.from({ length: count }, (_, c) => categories?.[c] ?? String(c + 1))
      return { canvas: drawPlot(title, labels, series, lo, hi), warnings, classDefs: {} }
    }
    const rows: ListRow[] = []
    for (let c = 0; c < count; c++) {
      series.forEach((s, k) => {
        if (s.values[c] === undefined) return
        const cells = scale(s.values[c])
        const bar = cells === 0 ? '░'.repeat(BAR_W) : barOf(BAR_W, 0, cells, FILLS[k % FILLS.length])
        rows.push({ label: k === 0 ? (categories?.[c] ?? String(c + 1)) : '', bar, suffix: String(s.values[c]) })
      })
    }
    // More than one series: say which fill is which.
    if (series.length > 1) rows.push({ label: series.map((s, k) => `${FILLS[k % FILLS.length]} ${s.name}`).join('  '), section: true })
    return { canvas: drawRows(title, rows), warnings, classDefs: {} }
  },
}
