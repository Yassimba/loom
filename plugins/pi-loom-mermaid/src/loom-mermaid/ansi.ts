import { type ClassStyle, resolveClassStyle } from './class-style.ts'
import type { MermaidArt, Role } from './types.ts'

const ESC = String.fromCharCode(27)
const OSC8 = `${ESC}]8;;`
const ST = `${ESC}\\`

/**
 * SGR parameter per role, e.g. `'2'` for dim, `'36'` for cyan,
 * `'38;5;244'` for a 256-colour index. A role left out is printed unstyled.
 */
export type AnsiTheme = Partial<Record<Role, string>>

/** Dim frame, plain labels, cyan connectors. Readable on light and dark. */
export const DEFAULT_THEME: AnsiTheme = {
  border: '2',
  edge: '36',
  edgeLabel: '2;36',
  title: '1',
}

const rgb = (hex: string, sgr: 38 | 48): string =>
  `${sgr};2;${[1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16)).join(';')}`

/**
 * The truecolor SGR a class style gives a span of the given role, or
 * undefined when the style says nothing about it (fall back to the theme).
 * Only `stroke` colors box borders. Node fills and text colors stay with
 * the terminal theme instead of overriding it.
 * A style that colors nothing for this role keeps `fallback` (the theme's
 * SGR), so a bold-only class bolds the themed look instead of replacing it.
 */
export function classSgr(st: ClassStyle, role: Role, fallback?: string): string | undefined {
  const color = role === 'border' && st.stroke !== undefined ? rgb(st.stroke, 38) : fallback
  const p = [...(st.bold === true ? ['1'] : []), ...(color === undefined ? [] : [color])]
  return p.length > 0 ? p.join(';') : undefined
}

/**
 * Render art to ANSI-coloured lines. Spans that carry author classes are
 * styled from `art.classDefs` (best effort — see `resolveClassStyle`),
 * overriding the role theme; everything else follows `theme`.
 *
 * A convenience over mapping `art.styled` yourself — reach for that directly
 * when your TUI has its own styling model.
 */
export function toAnsi(art: MermaidArt, theme: AnsiTheme = DEFAULT_THEME): string[] {
  return art.styled.map((row) =>
    row
      .map((span) => {
        const cls = resolveClassStyle(span.classes, art.classDefs)
        const sgr = cls !== null ? classSgr(cls, span.role, theme[span.role]) : theme[span.role]
        const text = sgr === undefined ? span.text : `${ESC}[${sgr}m${span.text}${ESC}[0m`
        // OSC 8 hyperlink around the whole run; terminals without support
        // ignore the sequences and print the text unchanged.
        return span.href === undefined ? text : `${OSC8}${span.href}${ST}${text}${OSC8}${ST}`
      })
      .join(''),
  )
}
