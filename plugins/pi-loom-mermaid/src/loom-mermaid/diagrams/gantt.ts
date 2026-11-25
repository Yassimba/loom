/**
 * `gantt`: tasks as bars on one shared time axis, a row each under its
 * section, with the dates written after the bar. Reads dates in the
 * diagram's `dateFormat` (year, month, day, hour, minute, second tokens),
 * `after` references and durations. Lenient: an unreadable statement is
 * dropped and recorded in `warnings`.
 */

import { MAX_EDGES } from '../graph.ts'
import { decodeHtmlEntities, fitLabel } from '../labels.ts'
import type { Diagram } from '../registry.ts'
import { headerKind, nonEmpty, statementsOf, words } from '../statements.ts'
import { afterKeyword, barOf, drawRows, type ListRow } from './rows.ts'

/** Columns of the time axis. */
const AXIS_W = 40
const UNIT_MS: Record<string, number> = {
  ms: 1,
  s: 1000,
  m: 60_000,
  h: 3_600_000,
  d: 86_400_000,
  w: 604_800_000,
  M: 2_592_000_000,
  y: 31_536_000_000,
}
const TAGS = ['active', 'done', 'crit', 'milestone']
/** Statements that style mermaid's own axis and mean nothing here. */
const IGNORED = ['excludes', 'includes', 'todaymarker', 'axisformat', 'tickinterval', 'weekday', 'weekend', 'acctitle', 'accdescr', 'inclusiveenddates', 'toplabelposition', 'click']

interface Task {
  name: string
  start: number
  end: number
  milestone: boolean
  section?: undefined
}

function parseDate(text: string, format: string): number | null {
  const tokens: string[] = []
  const pattern = format
    .replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    .replace(/YYYY|MM|DD|HH|mm|ss/g, (token) => {
      tokens.push(token)
      return token === 'YYYY' ? '(\\d{4})' : '(\\d{1,2})'
    })
  const match = new RegExp(`^${pattern}$`).exec(text)
  if (match === null) return null
  const part = (token: string, fallback: number): number => {
    const at = tokens.indexOf(token)
    return at === -1 ? fallback : Number(match[at + 1])
  }
  return Date.UTC(part('YYYY', 1970), part('MM', 1) - 1, part('DD', 1), part('HH', 0), part('mm', 0), part('ss', 0))
}

function parseDuration(text: string): number | null {
  const match = /^(\d+(?:\.\d+)?)(ms|s|m|h|d|w|M|y)$/.exec(text)
  return match === null ? null : Number(match[1]) * UNIT_MS[match[2]]
}

export const gantt: Diagram = {
  kind: 'gantt',
  headers: ['gantt'],
  render(src, limits) {
    const statements = statementsOf(src)
    if (headerKind(statements) !== 'gantt') return null
    let title: string | null = null
    let format = 'YYYY-MM-DD'
    const entries: (Task | { section: string })[] = []
    const ends = new Map<string, number>()
    const warnings: string[] = []
    let prevEnd: number | null = null

    for (const st of statements.slice(1, MAX_EDGES + 1)) {
      const first = words(st)[0]?.toLowerCase() ?? ''
      if (first === 'title') title = nonEmpty(afterKeyword(st))
      else if (first === 'dateformat') format = afterKeyword(st) || format
      else if (first === 'section') entries.push({ section: fitLabel(afterKeyword(st), limits.label) })
      else if (!IGNORED.includes(first)) {
        const task = parseTask(st, format, prevEnd, ends)
        if (task === null) {
          warnings.push(`dropped, unreadable statement: "${st}"`)
          continue
        }
        if (task.id !== null) ends.set(task.id, task.end)
        prevEnd = task.end
        entries.push({ ...task, name: fitLabel(task.name, limits.label) })
      }
    }
    const tasks = entries.filter((e): e is Task => e.section === undefined)
    if (tasks.length === 0) return null

    const from = Math.min(...tasks.map((t) => t.start))
    const span = Math.max(1, Math.max(...tasks.map((t) => t.end)) - from)
    const col = (time: number): number => Math.round(((time - from) / span) * AXIS_W)
    const stamp = (time: number): string => {
      const iso = new Date(time).toISOString()
      return /[Hms]/.test(format) ? `${iso.slice(0, 10)} ${iso.slice(11, 16)}` : iso.slice(0, 10)
    }
    const rows: ListRow[] = entries.map((e) =>
      e.section !== undefined
        ? { label: e.section, section: true }
        : {
            label: e.name,
            bar: barOf(AXIS_W, col(e.start), e.milestone ? 1 : col(e.end) - col(e.start), e.milestone ? '◆' : '█'),
            suffix: e.milestone ? stamp(e.start) : `${stamp(e.start)} → ${stamp(e.end)}`,
          },
    )
    return { canvas: drawRows(title, rows), warnings, classDefs: {} }
  },
}

/**
 * `Name : [tags,] [id,] [start,] end`. One field is the end or a duration
 * (the task starts where the last one ended), two are start and end, three
 * lead with the task's id.
 */
function parseTask(
  st: string,
  format: string,
  prevEnd: number | null,
  ends: Map<string, number>,
): (Omit<Task, 'section'> & { id: string | null }) | null {
  const colon = st.indexOf(':')
  if (colon <= 0) return null
  const name = decodeHtmlEntities(st.slice(0, colon).trim())
  const fields = st
    .slice(colon + 1)
    .split(',')
    .map((f) => f.trim())
  let milestone = false
  while (fields.length > 0 && TAGS.includes(fields[0])) milestone = fields.shift() === 'milestone' || milestone
  if (name === '' || fields.length === 0 || fields.length > 3 || fields.some((f) => f === '')) return null
  const id = fields.length === 3 ? (fields.shift() as string) : null
  const startText = fields.length === 2 ? (fields.shift() as string) : null
  let start = prevEnd ?? 0
  if (startText !== null) {
    const after = /^after\s+(.+)$/.exec(startText)
    const refs = after === null ? [] : words(after[1]).map((ref) => ends.get(ref))
    const at = after === null ? parseDate(startText, format) : refs.includes(undefined) ? null : Math.max(...(refs as number[]))
    if (at === null) return null
    start = at
  }
  const length = parseDuration(fields[0])
  const end = length === null ? parseDate(fields[0], format) : start + length
  if (end === null || end < start) return null
  return { name, start, end, milestone, id }
}
