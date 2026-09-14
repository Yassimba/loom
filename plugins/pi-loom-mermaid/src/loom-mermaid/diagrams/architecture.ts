/**
 * `architecture-beta`: services and junctions inside nested groups.
 *
 * Built-in icons use stable Unicode stand-ins; custom Iconify names remain
 * visible as text because a terminal cannot draw their SVGs.
 */

import { Graph, MAX_GROUP_DEPTH, MAX_GROUPS } from '../graph.ts'
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

// Icon and title are both optional: the spec's own group-edge example
// declares `service server[Server] in groupOne`.
const DECLARATION =
  /^(group|service)\s+([^\s()[\]:{}]+)\s*(?:\(([^)]*)\))?\s*(?:\[([^\]]*)\])?(?:\s+in\s+([^\s]+))?$/i
const JUNCTION = /^junction\s+([^\s:{}]+)(?:\s+in\s+([^\s]+))?$/i
const EDGE = /^([^\s:{}]+)(?:\{group\})?:([TBLR])\s*(<)?--(>)?\s*([TBLR]):([^\s:{}]+)(?:\{group\})?$/i
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

  // Architecture has no global direction; `orientEdges` picks one from the
  // ports once every edge is known. LR is the default: it gives grouped
  // boundary edges the existing router's more precise inner-node anchors.
  const graph = new Graph('right')
  const groupIndex = new Map<string, number>()
  /** Per edge, the port it leaves from (`T|B|L|R`). */
  const exits: string[] = []

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
      const [, fromId, fromPort, leftArrow, rightArrow, , toId] = edge
      const from = graph.index.get(fromId)
      const to = graph.index.get(toId)
      if (from === undefined || to === undefined) {
        graph.drop(st)
      } else if (
        graph.pushEdge({
          from,
          to,
          label: null,
          headFrom: leftArrow ? 'arrow' : 'none',
          headTo: rightArrow ? 'arrow' : 'none',
          line: 'solid',
        })
      ) {
        exits.push(fromPort.toUpperCase())
      }
      // `align row|column` pins coordinates for Mermaid's force-directed
      // layout; the layered engine here decides placement itself.
    } else if (!['title', 'align'].includes(firstWord(st).toLowerCase())) {
      graph.drop(st)
    }

    if (graph.truncated !== null) {
      graph.warnings.push(`diagram truncated: ${graph.truncated}`)
      break
    }
  }

  orientEdges(graph, exits)
  return graph.nodes.length === 0 ? null : graph
}

/**
 * Mermaid's ports place nodes: `a:B --> T:b` puts `b` below `a`, `a:L --
 * R:b` puts `b` to its left. The layered engine has one flow axis, so the
 * diagram runs the way most ports point, and an edge leaving against the
 * flow is stored reversed so its target still lands on the side the
 * author asked for. Ports across the flow axis say nothing about order and
 * are ignored; routing them literally would circle around the target.
 */
function orientEdges(graph: Graph, exits: string[]): void {
  const verticalCount = exits.filter((p) => p === 'T' || p === 'B').length
  const down = verticalCount > exits.length - verticalCount
  if (down) graph.dir = 'down'
  const against = down ? 'T' : 'L'
  graph.edges.forEach((e, i) => {
    if (exits[i] !== against) return
    ;[e.from, e.to] = [e.to, e.from]
    ;[e.headFrom, e.headTo] = [e.headTo, e.headFrom]
  })
}

function iconLabel(icon: string | undefined, rawLabel: string): string {
  const label = cleanLabel(rawLabel)
  if (icon === undefined) return label
  const name = cleanLabel(icon)
  const mark = ICONS[name.toLowerCase()] ?? `[${name.split(':').at(-1)}]`
  return `${mark} ${label}`
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
