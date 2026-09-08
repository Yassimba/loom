import type { Canvas } from './canvas.ts'
import { LIMITS, stripControls } from './labels.ts'
import { type Diagram, diagramFor } from './registry.ts'
import { frontmatterTitle } from './statements.ts'
import type { MermaidArt } from './types.ts'
import { stringWidth } from './width.ts'

export { type AnsiTheme, classSgr, DEFAULT_THEME, toAnsi } from './ansi.ts'
export { type ClassStyle, contrastOn, resolveClassStyle } from './class-style.ts'
export { type DiagramKind, diagramKind } from './registry.ts'
export { sourceBox } from './source-box.ts'
export type { MermaidArt, Role, Span } from './types.ts'

/**
 * Render a Mermaid source block as Unicode box-drawing art.
 *
 * Supported: `architecture-beta`, `graph`/`flowchart` (including `subgraph`),
 * `stateDiagram`, `classDiagram`, `erDiagram`, `sequenceDiagram`, `pie`,
 * `mindmap`, `timeline` and `gitGraph`.
 *
 * The diagram is laid out at whatever size it needs; `art.width` reports the
 * columns that turned out to be. Given `maxWidth`, a diagram wider than that
 * is laid out again with progressively tighter label limits. LR flowcharts
 * without explicit group directions or cross-scope member edges then retry
 * top-down before collapsing subgraphs. The first fit is returned; the source
 * is never rewritten.
 * Deciding what to do when even the final fallback exceeds the
 * space at hand is the caller's — `sourceBox` is the usual answer:
 *
 * ```ts
 * const art = render(src, { maxWidth: cols })
 * show(art && art.width <= cols ? art : sourceBox(src, cols))
 * ```
 *
 * `null` means there is no art to show: blank input, a diagram type this
 * renderer does not draw, a source in which not one statement parsed, or a
 * diagram large enough that laying it out is refused. `diagramKind` separates
 * the middle two.
 *
 * Rendering is best-effort in every grammar: a statement either contributes
 * what parsed or is dropped, and a diagram over a size cap renders its prefix.
 * Everything given up on is listed in `art.warnings` — advisory only, never a
 * reason to withhold the art.
 */
export function render(src: string, options: { maxWidth?: number } = {}): MermaidArt | null {
  src = stripControls(src)
  if (src.trim() === '') return null
  const diagram = diagramFor(src)
  if (diagram === null) return null
  // Preserve the requested direction while tightening labels, then try TD
  // before the existing collapsed fallback. Each attempt parses fresh source.
  let drawn: ReturnType<Diagram['render']> = null
  let art: ReturnType<Canvas['toLines']> = { plain: [], styled: [], width: 0 }
  let collapsed = false
  fitting: for (const [draw, collapse] of [
    [diagram.render, false],
    [diagram.renderDown, false],
    [diagram.render, true],
  ] as const) {
    if (draw === undefined) continue
    for (const limits of LIMITS) {
      if ((limits.collapse === true) !== collapse) continue
      const candidate = draw(src, limits)
      if (candidate === null) {
        if (draw === diagram.renderDown) break
        return null
      }
      drawn = candidate
      collapsed = collapse
      art = drawn.canvas.toLines()
      if (options.maxWidth === undefined || art.width <= options.maxWidth) break fitting
    }
  }
  if (drawn === null) return null
  if (collapsed && /^\s*subgraph\b|^\s*state\s+\S+\s*\{/m.test(src)) {
    drawn.warnings.push('too wide for the space: subgraphs drawn collapsed, one box each')
  }

  // A frontmatter `title:` is centred above the art, in the `title` role.
  const title = frontmatterTitle(src)
  if (title !== null) {
    const tw = stringWidth(title)
    art.width = Math.max(art.width, tw)
    const pad = ' '.repeat(Math.floor((art.width - tw) / 2))
    art.plain.unshift(pad + title, '')
    art.styled.unshift(
      pad === ''
        ? [{ text: title, role: 'title' }]
        : [
            { text: pad, role: 'none' },
            { text: title, role: 'title' },
          ],
      [],
    )
  }
  return { ...art, classDefs: drawn.classDefs, warnings: drawn.warnings }
}
