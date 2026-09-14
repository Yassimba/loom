/**
 * Shared layout geometry: padding, saturating math, placed boxes, edge text.
 */
import type { Edge } from './graph.ts'
import { stringWidth } from './width.ts'

/** Cells of padding between a box border and its text. */
export const PAD = 1
/** Minimum horizontal / vertical space between boxes. */
export const GAP_X = 3
export const GAP_Y = 2
/** Refuse to allocate a canvas larger than this many cells. */
export const MAX_CANVAS_CELLS = 1 << 21

/** Saturating subtraction; Rust's `usize` arithmetic never goes negative. */
export const sat = (a: number, b: number): number => Math.max(0, a - b)
export const half = (n: number): number => Math.floor(n / 2)

/**
 * Rows from a box's top to the row its edges meet. An even box has no
 * middle row, so the arrow takes the one above centre: the box then hangs
 * a row lower than the arrow instead of two rows higher, which reads as
 * centred on it.
 */
export const mid = (h: number): number => half(h) - (h % 2 === 0 ? 1 : 0)

/** Columns a label takes once fitted to `max`. */
export const labelCols = (text: string, max: number): number => Math.min(stringWidth(text), max)

/** Where a label starts: right of its arrow, or ending just left of it. */
export const labelStart = (arrowX: number, text: string, left: boolean, max: number): number =>
  left ? sat(arrowX, labelCols(text, max) + 1) : arrowX + 2

/** Everything an edge says, joined: source cardinality, verb, target cardinality. */
export function edgeText(edge: Edge): string | null {
  const joined = [edge.cardFrom ?? '', edge.label ?? '', edge.cardTo ?? '']
    .filter((part) => part !== '')
    .join(' ')
  return joined === '' ? null : joined
}

export interface Placed {
  x: number
  y: number
  w: number
  h: number
  cx: number
  cy: number
  rank: number
}
