/**
 * `quadrantChart`: a two-by-two grid with each quadrant named in its
 * corner and the points plotted on it. Lenient: an unreadable statement is
 * dropped and recorded in `warnings`.
 */

import { Canvas, drawText } from '../canvas.ts'
import { MAX_NODES } from '../graph.ts'
import { cleanLabel, fitLabel } from '../labels.ts'
import type { Diagram } from '../registry.ts'
import { headerKind, nonEmpty, statementsOf, words } from '../statements.ts'
import { stringWidth } from '../width.ts'
import { afterKeyword } from './rows.ts'

/** Inner cells of one quadrant. */
const QW = 24
const QH = 6

export const quadrant: Diagram = {
  kind: 'quadrant',
  headers: ['quadrantchart'],
  render(src) {
    const statements = statementsOf(src)
    if (headerKind(statements) !== 'quadrantchart') return null
    let title: string | null = null
    const axis: Record<string, [string, string]> = { 'x-axis': ['', ''], 'y-axis': ['', ''] }
    const names = ['', '', '', '']
    const points: { name: string; x: number; y: number }[] = []
    const warnings: string[] = []

    for (const st of statements.slice(1)) {
      const first = words(st)[0]?.toLowerCase() ?? ''
      const corner = /^quadrant-([1-4])$/.exec(first)
      if (first === 'title') title = nonEmpty(afterKeyword(st))
      else if (first in axis) {
        const [low, high] = afterKeyword(st).split('-->').map((part) => cleanLabel(part.trim()))
        axis[first] = [low ?? '', high ?? '']
      } else if (corner !== null) names[Number(corner[1]) - 1] = cleanLabel(afterKeyword(st))
      else {
        // `Name: [x, y]`, both in 0..1; styling after the bracket is ignored.
        const point = /^(.*?)(?::::\S+)?\s*:\s*\[\s*([\d.]+)\s*,\s*([\d.]+)\s*\]/.exec(st)
        const [x, y] = point === null ? [Number.NaN, Number.NaN] : [Number(point[2]), Number(point[3])]
        if (point === null || !(x >= 0 && x <= 1 && y >= 0 && y <= 1)) {
          if (!['classdef', 'acctitle', 'accdescr'].includes(first)) warnings.push(`dropped, unreadable statement: "${st}"`)
        } else if (points.length < MAX_NODES) points.push({ name: cleanLabel(point[1].trim()), x, y })
      }
    }
    if (points.length === 0 && names.every((n) => n === '')) return null

    const gridW = 2 * QW + 3
    const gridH = 2 * QH + 3
    const top = (title === null ? 0 : 1) + (axis['y-axis'][1] === '' ? 0 : 1)
    const [yLow, xLow, xHigh] = [axis['y-axis'][0], axis['x-axis'][0], axis['x-axis'][1]]
    const height = top + gridH + (yLow === '' ? 0 : 1) + (xLow === '' && xHigh === '' ? 0 : 1)
    const canvas = new Canvas(Math.max(gridW, title === null ? 0 : stringWidth(title)), height)
    if (title !== null) drawText(canvas, title, Math.floor((canvas.w - stringWidth(title)) / 2), 0, 'title')
    if (axis['y-axis'][1] !== '') drawText(canvas, `↑ ${fitLabel(axis['y-axis'][1], gridW - 2)}`, 0, top - 1, 'edgeLabel')

    // The frame and the two axes through its middle.
    for (let row = 0; row < gridH; row++) {
      const rule = row === 0 || row === QH + 1 || row === gridH - 1
      const [l, m, r] = row === 0 ? '┌┬┐' : row === QH + 1 ? '├┼┤' : row === gridH - 1 ? '└┴┘' : '│││'
      const line = rule ? '─'.repeat(QW) : ' '.repeat(QW)
      drawText(canvas, l + line + m + line + r, 0, top + row, 'border')
    }
    // Mermaid numbers the quadrants from the top right, anticlockwise.
    const corners: [number, number][] = [
      [QW + 3, 1],
      [2, 1],
      [2, QH + 2],
      [QW + 3, QH + 2],
    ]
    names.forEach((name, i) => {
      if (name !== '') drawText(canvas, fitLabel(name, QW - 2), corners[i][0], top + corners[i][1], 'title')
    })
    for (const point of points) {
      const x = 1 + Math.round(point.x * (gridW - 3))
      const y = top + 1 + Math.round((1 - point.y) * (gridH - 3))
      const label = fitLabel(point.name, QW - 2)
      const right = x + 2 + stringWidth(label) <= gridW - 1
      drawText(canvas, '●', x, y, 'edge')
      drawText(canvas, label, right ? x + 2 : x - 1 - stringWidth(label), y, 'text')
    }
    let row = top + gridH
    if (yLow !== '') drawText(canvas, `↓ ${fitLabel(yLow, gridW - 2)}`, 0, row++, 'edgeLabel')
    if (xLow !== '') drawText(canvas, `← ${fitLabel(xLow, QW - 2)}`, 0, row, 'edgeLabel')
    if (xHigh !== '') {
      const text = `${fitLabel(xHigh, QW - 2)} →`
      drawText(canvas, text, gridW - stringWidth(text), row, 'edgeLabel')
    }
    return { canvas, warnings, classDefs: {} }
  },
}
