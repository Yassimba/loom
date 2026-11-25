/**
 * The list layout the chart-like diagrams share: an optional title, then
 * rows of label, bar and trailing text in aligned columns, with section
 * headers on rows of their own.
 */

import { Canvas, drawText } from '../canvas.ts'
import { stringWidth } from '../width.ts'

export interface ListRow {
  label: string
  /** Fill and track glyphs; a track cell (`░`, blank) paints as border. */
  bar?: string
  suffix?: string
  /** A section header: the label alone, flush left. */
  section?: boolean
}

/** A bar of `width` cells: `fill` from `from` for `length`, track elsewhere. */
export function barOf(width: number, from: number, length: number, fill = '█'): string {
  const start = Math.max(0, Math.min(width - 1, from))
  const len = Math.max(1, Math.min(width - start, length))
  return '░'.repeat(start) + fill.repeat(len) + '░'.repeat(width - start - len)
}

export function drawRows(title: string | null, rows: ListRow[]): Canvas {
  const items = rows.filter((r) => !r.section)
  const indent = rows.some((r) => r.section) ? 2 : 0
  const labelW = Math.max(0, ...items.map((r) => stringWidth(r.label)))
  const barW = Math.max(0, ...items.map((r) => stringWidth(r.bar ?? '')))
  const barX = indent + labelW + 2
  const suffixX = barX + (barW === 0 ? 0 : barW + 2)
  const width = Math.max(
    title === null ? 0 : stringWidth(title),
    ...rows.map((r) => (r.section ? stringWidth(r.label) : suffixX + stringWidth(r.suffix ?? ''))),
  )
  const top = title === null ? 0 : 1
  const canvas = new Canvas(width, top + rows.length)
  if (title !== null) drawText(canvas, title, Math.floor((width - stringWidth(title)) / 2), 0, 'title')
  rows.forEach((row, i) => {
    const y = top + i
    if (row.section) {
      drawText(canvas, row.label, 0, y, 'title')
      return
    }
    drawText(canvas, row.label, indent, y, 'text')
    let x = barX
    for (const ch of row.bar ?? '') {
      drawText(canvas, ch, x, y, ch === '░' || ch === ' ' ? 'border' : 'edge')
      x += stringWidth(ch)
    }
    if (row.suffix) drawText(canvas, row.suffix, suffixX, y, 'edgeLabel')
  })
  return canvas
}

/** The text after a leading keyword, quotes peeled: `title "Sales"` is `Sales`. */
export const afterKeyword = (st: string): string =>
  st
    .trim()
    .replace(/^\S+\s*/, '')
    .trim()
    .replace(/^"(.*)"$/, '$1')
