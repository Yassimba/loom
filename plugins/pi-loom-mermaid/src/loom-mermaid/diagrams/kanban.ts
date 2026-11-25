/**
 * `kanban`: columns side by side, each a heading over its cards. Indentation
 * is the grammar: the shallowest lines are columns, deeper ones their cards.
 * Lenient: an unreadable line is dropped and recorded in `warnings`.
 */

import { Canvas, drawText } from '../canvas.ts'
import { MAX_NODES } from '../graph.ts'
import { cleanLabel, srcLines, wrapLabel } from '../labels.ts'
import type { Diagram } from '../registry.ts'
import { frontmatterEnd } from '../statements.ts'
import { stringWidth } from '../width.ts'

const GAP = 2

/** `id[Text]`, `[Text]` or bare text, with any `@{ … }` metadata dropped. */
function cardText(body: string): string {
  const plain = body.replace(/@\{.*\}\s*$/, '').trim()
  const bracket = /^[^[]*\[(.*)\]$/.exec(plain)
  return cleanLabel(bracket === null ? plain : bracket[1])
}

export const kanban: Diagram = {
  kind: 'kanban',
  headers: ['kanban'],
  render(src, limits) {
    const parsed = parseKanban(src)
    if (parsed === null) return null
    const { columns, warnings } = parsed
    const cardW = limits.wrap
    const blocks = columns.map((column) => {
      const cards = column.cards.map((card) => wrapLabel(card, cardW, 3))
      const w = Math.max(stringWidth(column.name), ...cards.flat().map((line) => stringWidth(line) + 2))
      return { name: column.name, cards, w }
    })
    const height = 2 + Math.max(0, ...blocks.map((b) => b.cards.flat().length))
    const width = blocks.reduce((sum, b) => sum + b.w, 0) + GAP * (blocks.length - 1)
    const canvas = new Canvas(width, height)
    let x = 0
    for (const block of blocks) {
      drawText(canvas, block.name, x, 0, 'title')
      drawText(canvas, '─'.repeat(block.w), x, 1, 'edge')
      let y = 2
      for (const card of block.cards) {
        card.forEach((line, i) => {
          if (i === 0) drawText(canvas, '•', x, y, 'edge')
          drawText(canvas, line, x + 2, y++, 'text')
        })
      }
      x += block.w + GAP
    }
    return { canvas, warnings, classDefs: {} }
  },
}

function parseKanban(src: string): { columns: { name: string; cards: string[] }[]; warnings: string[] } | null {
  const lines = srcLines(src).slice(frontmatterEnd(srcLines(src)))
  const headerAt = lines.findIndex((l) => l.trim() !== '')
  if (headerAt === -1 || lines[headerAt].trim().toLowerCase() !== 'kanban') return null
  const body = lines
    .slice(headerAt + 1)
    .map((raw) => raw.split('%%')[0])
    .filter((line) => line.trim() !== '' && !line.trim().startsWith('@{'))
    .slice(0, MAX_NODES)
  const depth = (line: string): number => line.length - line.trimStart().length
  const top = Math.min(...body.map(depth))
  const columns: { name: string; cards: string[] }[] = []
  const warnings: string[] = []
  for (const line of body) {
    const text = cardText(line.trim())
    if (text === '') warnings.push(`dropped, unreadable statement: "${line.trim()}"`)
    else if (depth(line) === top) columns.push({ name: text, cards: [] })
    else columns.at(-1)?.cards.push(text)
  }
  return columns.length === 0 ? null : { columns, warnings }
}
