/**
 * The C4 diagrams (`C4Context`, `C4Container`, `C4Component`, `C4Dynamic`,
 * `C4Deployment`): elements as boxes naming their kind, boundaries as
 * frames, relations as labelled arrows. C4 is a vocabulary over boxes and
 * arrows, so the source is rewritten as a flowchart and drawn by that
 * renderer. Lenient: an unreadable statement is dropped and recorded in
 * `warnings`.
 *
 * shortcut: descriptions, technologies and `title` are not drawn; add them
 * to the box text here if the boxes should carry them.
 */

import type { Diagram } from '../registry.ts'
import { headerKind, splitTop, statementsOf } from '../statements.ts'
import { flowchart } from './flowchart.ts'

/** Macros that style or lay out mermaid's own drawing. */
const STYLING = /^(Update|Add|Show|Hide|Lay|title\b|accTitle|accDescr)/

/** A label safe inside a flowchart's quoted text. */
const quoted = (text: string): string => text.replace(/"/g, "'").replace(/\|/g, '/')

export const c4: Diagram = {
  kind: 'c4',
  headers: ['c4context', 'c4container', 'c4component', 'c4dynamic', 'c4deployment'],
  render(src, limits) {
    const statements = statementsOf(src)
    const header = headerKind(statements)
    if (header === null || !c4.headers.includes(header)) return null
    const out = ['flowchart TD']
    const dropped: string[] = []
    let open = 0

    for (const st of statements.slice(1)) {
      if (st === '}') {
        if (open > 0) out.push('end')
        open = Math.max(0, open - 1)
        continue
      }
      const call = /^(\w+)\s*\((.*)\)\s*(\{)?$/.exec(st)
      if (call === null || STYLING.test(st)) {
        if (!STYLING.test(st)) dropped.push(st)
        continue
      }
      const [, macro, body, brace] = call
      // `$link=…` and friends are named styling arguments.
      const args = splitTop(body, (c) => c === ',')
        .map((arg) => arg.trim())
        .filter((arg) => !arg.startsWith('$'))
        .map((arg) => arg.replace(/^"(.*)"$/, '$1'))
      if (/^(Bi)?Rel/.test(macro)) {
        // `RelIndex` leads with a step number; `Rel_Back` points the other way.
        const [a, b, label] = macro === 'RelIndex' ? args.slice(1) : args
        if (a === undefined || b === undefined) dropped.push(st)
        else {
          const [from, to] = macro.endsWith('Back') ? [b, a] : [a, b]
          const text = label ? `|"${quoted(label)}"|` : ''
          out.push(`${from} ${macro.startsWith('Bi') ? '<-->' : '-->'}${text} ${to}`)
        }
        continue
      }
      const [id, label] = args
      if (id === undefined) {
        dropped.push(st)
        continue
      }
      if (brace !== undefined) {
        out.push(`subgraph ${id}["${quoted(label ?? id)}"]`)
        open++
        continue
      }
      const kind = macro.replace(/_/g, ' ').toLowerCase()
      const text = `"«${kind}»<br/>${quoted(label ?? id)}"`
      out.push(/db$/i.test(macro.replace(/_Ext$/, '')) ? `${id}[(${text})]` : `${id}[${text}]`)
    }
    while (open-- > 0) out.push('end')
    const drawn = flowchart.render(out.join('\n'), limits)
    if (drawn === null) return null
    drawn.warnings.push(...dropped.map((st) => `dropped, unreadable statement: "${st}"`))
    return drawn
  },
}
