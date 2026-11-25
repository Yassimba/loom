/**
 * `journey`: a user journey as a scored list, one row per task under its
 * section, the 1–5 score as a bar and the actors named after it. Lenient: an
 * unreadable statement is dropped and recorded in `warnings`.
 */

import { MAX_EDGES } from '../graph.ts'
import { decodeHtmlEntities, fitLabel } from '../labels.ts'
import type { Diagram } from '../registry.ts'
import { headerKind, nonEmpty, statementsOf, words } from '../statements.ts'
import { afterKeyword, barOf, drawRows, type ListRow } from './rows.ts'

export const journey: Diagram = {
  kind: 'journey',
  headers: ['journey'],
  render(src, limits) {
    const statements = statementsOf(src)
    if (headerKind(statements) !== 'journey') return null
    let title: string | null = null
    const rows: ListRow[] = []
    const warnings: string[] = []
    for (const st of statements.slice(1, MAX_EDGES + 1)) {
      const first = words(st)[0]?.toLowerCase()
      if (first === 'title') title = nonEmpty(afterKeyword(st))
      else if (first === 'section') rows.push({ label: fitLabel(afterKeyword(st), limits.label), section: true })
      else {
        // `Task name: score: actor, actor`
        const [name, score, actors] = st.split(':').map((part) => decodeHtmlEntities(part.trim()))
        const value = Number(score)
        if (!name || !Number.isFinite(value) || score === '') {
          warnings.push(`dropped, unreadable statement: "${st}"`)
          continue
        }
        const shown = Math.max(0, Math.min(5, Math.round(value)))
        const bar = shown === 0 ? '░'.repeat(5) : barOf(5, 0, shown)
        rows.push({ label: fitLabel(name, limits.label), bar, suffix: [`${shown}/5`, actors ?? ''].join('  ').trim() })
      }
    }
    if (!rows.some((r) => !r.section)) return null
    return { canvas: drawRows(title, rows), warnings, classDefs: {} }
  },
}
