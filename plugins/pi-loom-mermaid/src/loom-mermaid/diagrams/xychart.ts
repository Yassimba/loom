/**
 * `xychart-beta`: every series as horizontal bars, a row per category, on
 * one shared value scale. A terminal draws a readable bar and no readable
 * line, so `line` series are bars too, told apart by their fill. Lenient:
 * an unreadable statement is dropped and recorded in `warnings`.
 */

import type { Diagram } from '../registry.ts'
import { headerKind, nonEmpty, statementsOf, words } from '../statements.ts'
import { afterKeyword, barOf, drawRows, type ListRow } from './rows.ts'

const BAR_W = 30
const FILLS = ['█', '▓', '▒']

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
    const series: { name: string; values: number[] }[] = []
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
