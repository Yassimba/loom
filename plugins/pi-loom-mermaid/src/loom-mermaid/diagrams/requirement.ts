/**
 * `requirementDiagram`: requirements and elements as boxes naming their
 * kind over their fields, relations as labelled dotted arrows. It is a
 * class diagram in other words, so the source is rewritten as one and
 * drawn by that renderer. Lenient: an unreadable statement is dropped and
 * recorded in `warnings`.
 */

import type { Diagram } from '../registry.ts'
import { headerKind, statementsOf } from '../statements.ts'
import { classDiagram } from './class.ts'

const KINDS = /^(requirement|functionalRequirement|interfaceRequirement|performanceRequirement|physicalRequirement|designConstraint|element)\s+(.+?)\s*\{$/i
const IGNORED = /^(classDef|class|style|accTitle|accDescr)\b/

export const requirement: Diagram = {
  kind: 'requirement',
  headers: ['requirementdiagram'],
  render(src, limits) {
    const statements = statementsOf(src)
    if (headerKind(statements) !== 'requirementdiagram') return null
    const out = ['classDiagram']
    const dropped: string[] = []
    // Names may be quoted and spaced; class ids may not.
    const ids = new Map<string, string>()
    const idOf = (name: string): string => {
      const key = name.trim().replace(/^"(.*)"$/, '$1')
      if (!ids.has(key)) ids.set(key, `r${ids.size}`)
      return ids.get(key) as string
    }
    let inBody = false

    for (const st of statements.slice(1)) {
      if (inBody) {
        inBody = st !== '}'
        out.push(inBody ? st.replace(/"/g, '') : '}')
        continue
      }
      const declared = KINDS.exec(st)
      // `a - satisfies -> b`, or the same read backwards: `b <- satisfies - a`.
      const forward = /^(.+?)\s+-\s+(\w+)\s+->\s+(.+)$/.exec(st)
      const backward = /^(.+?)\s+<-\s+(\w+)\s+-\s+(.+)$/.exec(st)
      if (declared !== null) {
        const name = declared[2].replace(/^"(.*)"$/, '$1')
        out.push(`class ${idOf(name)}["${name.replace(/"/g, "'")}"] {`, `<<${declared[1]}>>`)
        inBody = true
      } else if (forward !== null) out.push(`${idOf(forward[1])} ..> ${idOf(forward[3])} : ${forward[2]}`)
      else if (backward !== null) out.push(`${idOf(backward[3])} ..> ${idOf(backward[1])} : ${backward[2]}`)
      else if (/^direction\b/i.test(st)) out.push(st)
      else if (!IGNORED.test(st)) dropped.push(st)
    }
    const drawn = classDiagram.render(out.join('\n'), limits)
    if (drawn === null) return null
    drawn.warnings.push(...dropped.map((st) => `dropped, unreadable statement: "${st}"`))
    return drawn
  },
}
