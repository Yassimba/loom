/**
 * Entry points for the layered diagrams (flowchart, state, class, ER):
 * choose what each node box holds, lay the graph out, paint it, orient it.
 * Subgraphs recurse: each becomes a framed box holding its own canvas.
 */

import { Canvas } from './canvas.ts'
import type { Edge, Node } from './graph.ts'
import { Graph } from './graph.ts'
import { layout, type NodeExtra } from './layout.ts'
import { orient, paint } from './paint.ts'

/** A laid-out canvas, or `null` when the diagram is empty or over the cell cap. */
export type CanvasResult = Canvas | null

/** Flowchart and state diagrams: plain boxes, no extra content. */
export function layoutFlowchart(graph: Graph): CanvasResult {
  const extras: NodeExtra[] = graph.nodes.map(() => ({ kind: 'plain' }))
  const canvas = layoutCanvas(graph, extras)
  return canvas && orient(canvas, graph)
}

/** Class and ER diagrams: boxes divided into title / attribute / method rows. */
export function layoutClass(graph: Graph): CanvasResult {
  const extras: NodeExtra[] = graph.nodes.map((node) => ({
    kind: 'compartments',
    sections: node.sections ?? [[node.label]],
  }))
  const canvas = layoutCanvas(graph, extras)
  return canvas && orient(canvas, graph)
}

// -------------------------------------------------------------------- groups

/** An endpoint inside a scope: a plain node or a (proxied) subgraph. */
interface ScopeItem {
  group: boolean
  i: number
}

/**
 * Lay out a flowchart that uses `subgraph`.
 *
 * Each subgraph becomes a framed box holding its own independently laid-out
 * canvas. An edge is drawn in the innermost scope containing both endpoints;
 * one crossing a subgraph boundary attaches to the frame instead of the node.
 */
export function layoutGrouped(graph: Graph): CanvasResult {
  // A node whose id matches a subgraph id stands in for that subgraph.
  const proxy = new Map<number, number>()
  graph.groups.forEach((g, gi) => {
    const ni = graph.index.get(g.id)
    if (ni !== undefined) proxy.set(ni, gi)
  })

  const groupChain = (g: number | null): number[] => {
    const chain: number[] = []
    let cur = g
    while (cur !== null) {
      chain.push(cur)
      cur = graph.groups[cur].parent
    }
    return chain.reverse()
  }
  const endpoint = (n: number): { item: ScopeItem; chain: number[] } => {
    const gi = proxy.get(n)
    return gi === undefined
      ? { item: { group: false, i: n }, chain: groupChain(graph.nodeGroup[n]) }
      : { item: { group: true, i: gi }, chain: groupChain(graph.groups[gi].parent) }
  }

  /** Edges bucketed by the scope that draws them; `null` is the top level. */
  const scopeEdges = new Map<number | null, [ScopeItem, ScopeItem, number][]>()
  const referenced = new Array<boolean>(graph.groups.length).fill(false)
  graph.edges.forEach((e, ei) => {
    const f = endpoint(e.from)
    const t = endpoint(e.to)
    let k = 0
    while (k < f.chain.length && k < t.chain.length && f.chain[k] === t.chain[k]) k++
    const scope = k === 0 ? null : f.chain[k - 1]
    const fItem = f.chain.length > k ? { group: true, i: f.chain[k] } : f.item
    const tItem = t.chain.length > k ? { group: true, i: t.chain[k] } : t.item
    for (const item of [fItem, tItem]) {
      if (item.group) referenced[item.i] = true
    }
    const list = scopeEdges.get(scope)
    if (list) list.push([fItem, tItem, ei])
    else scopeEdges.set(scope, [[fItem, tItem, ei]])
  })

  const directNodes = new Map<number | null, number[]>()
  graph.nodeGroup.forEach((g, ni) => {
    if (proxy.has(ni)) return
    const list = directNodes.get(g)
    if (list) list.push(ni)
    else directNodes.set(g, [ni])
  })

  // Drop empty subgraphs, but keep any that an edge attaches to. Walked by
  // the actual child relation: state `--` regions reparent earlier groups
  // under later ones, so index order says nothing about depth.
  const childGroups: number[][] = graph.groups.map(() => [])
  graph.groups.forEach((g, gi) => {
    if (g.parent !== null) childGroups[g.parent].push(gi)
  })
  const keep = new Array<boolean>(graph.groups.length).fill(false)
  const visit = (gi: number): boolean => {
    let kept = referenced[gi] || (directNodes.get(gi) ?? []).length > 0
    for (const c of childGroups[gi]) if (visit(c)) kept = true
    keep[gi] = kept
    return kept
  }
  graph.groups.forEach((g, gi) => {
    if (g.parent === null) visit(gi)
  })

  const canvas = buildScope(graph, null, scopeEdges, directNodes, keep)
  return canvas && orient(canvas, graph)
}

function buildScope(
  graph: Graph,
  scope: number | null,
  scopeEdges: Map<number | null, [ScopeItem, ScopeItem, number][]>,
  directNodes: Map<number | null, number[]>,
  keep: boolean[],
): CanvasResult {
  const items: ScopeItem[] = (directNodes.get(scope) ?? []).map((i) => ({ group: false, i }))
  const childGroups = graph.groups
    .map((_, gi) => gi)
    .filter((gi) => graph.groups[gi].parent === scope && keep[gi])
  items.push(...childGroups.map((i) => ({ group: true, i })))

  if (items.length === 0) return new Canvas(1, 1)

  const nodeAt = new Map<number, number>()
  const groupAt = new Map<number, number>()
  const nodes: Node[] = []
  const extras: NodeExtra[] = []
  for (const item of items) {
    ;(item.group ? groupAt : nodeAt).set(item.i, nodes.length)
    if (!item.group) {
      nodes.push({
        label: graph.nodes[item.i].label,
        shape: graph.nodes[item.i].shape,
        classes: graph.nodes[item.i].classes,
        href: graph.nodes[item.i].href,
      })
      extras.push({ kind: 'plain' })
    } else {
      const sub = buildScope(graph, item.i, scopeEdges, directNodes, keep)
      if (sub === null) return null
      nodes.push({ label: graph.groups[item.i].label, shape: 'rect' })
      extras.push({ kind: 'frame', sub })
    }
  }

  const edges: Edge[] = []
  for (const [f, t, ei] of scopeEdges.get(scope) ?? []) {
    const fi = (f.group ? groupAt : nodeAt).get(f.i)
    const ti = (t.group ? groupAt : nodeAt).get(t.i)
    if (fi === undefined || ti === undefined) continue
    const e = graph.edges[ei]
    const collapsed: Edge = {
      from: fi,
      to: ti,
      label: e.label,
      headTo: e.headTo,
      headFrom: e.headFrom,
      line: e.line,
    }
    // Edges from (or to) different nodes inside one frame collapse onto
    // the frame and become indistinguishable; draw them once.
    const twin = (a: Edge, b: Edge): boolean =>
      a.from === b.from && a.to === b.to && a.label === b.label && a.headTo === b.headTo && a.headFrom === b.headFrom && a.line === b.line
    if ((f.group || t.group) && edges.some((x) => twin(x, collapsed))) continue
    edges.push(collapsed)
  }

  // Layout only reads nodes/edges/dir, so a bare Graph carrying those is enough.
  const synth = new Graph(graph.dir)
  synth.nodes = nodes
  synth.edges = edges
  return layoutCanvas(synth, extras)
}

/** Lay out and paint one scope. */
function layoutCanvas(graph: Graph, extras: NodeExtra[]): CanvasResult {
  const lay = layout(graph, extras)
  return lay === null ? null : paint(graph, extras, lay)
}

// ------------------------------------------------------------------- drawing
