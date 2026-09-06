import type { Canvas } from './canvas.ts'
import { LIMITS, stripControls, withLimits } from './labels.ts'
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
 * Supported: `graph`/`flowchart` (including `subgraph`), `stateDiagram`,
 * `classDiagram`, `erDiagram`, `sequenceDiagram`, `pie`, `mindmap`,
 * `timeline` and `gitGraph`.
 *
 * The diagram is laid out at whatever size it needs; `art.width` reports the
 * columns that turned out to be. Given `maxWidth`, a diagram wider than that
 * is laid out again with progressively tighter label limits and the first
 * fit is returned. Deciding what to do when even the tightest exceeds the
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
  // Too wide for the space given: lay out again with shorter labels and
  // tighter wrapping, tightest last, and keep the first that fits (else
  // the tightest, for the caller to judge against `art.width`).
  let drawn: ReturnType<Diagram['render']> = null
  let art: ReturnType<Canvas['toLines']> = { plain: [], styled: [], width: 0 }
  for (const limits of LIMITS) {
    drawn = withLimits(limits, () => diagram.render(src))
    if (drawn === null) return null
    art = drawn.canvas.toLines()
    if (options.maxWidth === undefined || art.width <= options.maxWidth) break
  }
  if (drawn === null) return null

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
