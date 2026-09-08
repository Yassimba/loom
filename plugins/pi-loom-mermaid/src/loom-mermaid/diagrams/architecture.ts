/**
 * `architecture-beta`: services and junctions inside nested groups.
 *
 * Built-in icons use stable Unicode stand-ins; custom Iconify names remain
 * visible as text because a terminal cannot draw their SVGs.
 */

import { Graph, MAX_GROUP_DEPTH, MAX_GROUPS, type PortSide } from '../graph.ts'
import { cleanLabel } from '../labels.ts'
import { layoutFlowchart, layoutGrouped } from '../graph-render.ts'
import type { Diagram } from '../registry.ts'
import { firstWord, headerKind, statementsOf } from '../statements.ts'

export const architecture: Diagram = {
  kind: 'architecture',
  headers: ['architecture-beta'],
  render(src, limits) {
    const graph = parseArchitecture(src)
    if (graph === null) return null
    const canvas = graph.groups.length === 0 ? layoutFlowchart(graph, limits) : layoutGrouped(graph, limits)
    if (canvas === null) return null
    return { canvas, warnings: graph.warnings, classDefs: {} }
  },
}

const DECLARATION = /^(group|service)\s+([^\s()[\]:{}]+)\(([^)]*)\)\[([^\]]*)\](?:\s+in\s+([^\s]+))?$/i
const JUNCTION = /^junction\s+([^\s:{}]+)(?:\s+in\s+([^\s]+))?$/i
const EDGE = /^([^\s:{}]+)(?:\{group\})?:([TBLR])\s*(<)?--(>)?\s*([TBLR]):([^\s:{}]+)(?:\{group\})?$/i
const SIDES: Record<string, PortSide> = { T: 'top', B: 'bottom', L: 'left', R: 'right' }
const ICONS: Record<string, string> = {
  cloud: '☁',
  database: '◉',
  disk: '▰',
  internet: '◎',
  server: '▣',
}

function parseArchitecture(src: string): Graph | null {
  const statements = statementsOf(src)
  if (headerKind(statements) !== 'architecture-beta') return null

  // Architecture has no global direction. LR gives grouped boundary edges the
  // existing router's more precise inner-node anchors.
  const graph = new Graph('right')
  const groupIndex = new Map<string, number>()

  for (const st of statements.slice(1)) {
    const declaration = st.match(DECLARATION)
    const junction = st.match(JUNCTION)
    const edge = st.match(EDGE)

    if (declaration) {
      const [, kind, id, icon, rawLabel, parentId] = declaration
      const parent = parentId === undefined ? null : groupIndex.get(parentId)
      if (parentId !== undefined && parent === undefined) {
        graph.drop(st)
      } else if (kind.toLowerCase() === 'group') {
        if (groupIndex.has(id)) {
          graph.drop(st)
        } else if (
          graph.groups.length >= MAX_GROUPS ||
          groupDepth(graph, parent ?? null) >= MAX_GROUP_DEPTH
        ) {
          graph.truncated ??= `subgraph cap (${MAX_GROUPS} groups, depth ${MAX_GROUP_DEPTH}) reached`
        } else {
          groupIndex.set(id, graph.groups.length)
          graph.groups.push({ id, label: iconLabel(icon, rawLabel || id), parent: parent ?? null })
        }
      } else {
        addNode(graph, id, iconLabel(icon, rawLabel || id), parent ?? null, 'rect')
      }
    } else if (junction) {
      const [, id, parentId] = junction
      const parent = parentId === undefined ? null : groupIndex.get(parentId)
      if (parentId !== undefined && parent === undefined) graph.drop(st)
      else addNode(graph, id, '•', parent ?? null, 'round')
    } else if (edge) {
      const [, fromId, fromPort, leftArrow, rightArrow, toPort, toId] = edge
      const from = graph.index.get(fromId)
      const to = graph.index.get(toId)
      if (from === undefined || to === undefined) {
        graph.drop(st)
      } else {
        graph.pushEdge({
          from,
          to,
          label: null,
          headFrom: leftArrow ? 'arrow' : 'none',
          headTo: rightArrow ? 'arrow' : 'none',
          line: 'solid',
          fromSide: SIDES[fromPort.toUpperCase()],
          toSide: SIDES[toPort.toUpperCase()],
        })
      }
    } else if (firstWord(st).toLowerCase() !== 'title') {
      graph.drop(st)
    }

    if (graph.truncated !== null) {
      graph.warnings.push(`diagram truncated: ${graph.truncated}`)
      break
    }
  }

  return graph.nodes.length === 0 ? null : graph
}

function iconLabel(icon: string, rawLabel: string): string {
  const name = cleanLabel(icon)
  const mark = ICONS[name.toLowerCase()] ?? `[${name.split(':').at(-1)}]`
  return `${mark} ${cleanLabel(rawLabel)}`
}

function addNode(
  graph: Graph,
  id: string,
  label: string,
  group: number | null,
  shape: 'rect' | 'round',
): void {
  const previous = graph.curGroup
  graph.curGroup = group
  graph.nodeIndex(id, label, shape)
  graph.curGroup = previous
}

function groupDepth(graph: Graph, parent: number | null): number {
  let depth = 0
  for (let at = parent; at !== null; at = graph.groups[at].parent) depth++
  return depth
}
