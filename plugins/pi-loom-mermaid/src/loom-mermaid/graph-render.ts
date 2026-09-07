/**
 * Entry points for the layered diagrams (flowchart, state, class, ER):
 * choose what each node box holds, lay the graph out, paint it, orient it.
 * Subgraphs recurse: each becomes a framed box holding its own canvas.
 */

import { Canvas } from './canvas.ts'
import type { Anchor, Edge, Node } from './graph.ts'
import { Graph } from './graph.ts'
import type { Limits } from './labels.ts'
import { layout, type NodeExtra } from './layout.ts'
import { frameOrigin, orient, paint } from './paint.ts'

/** A laid-out canvas, or `null` when the diagram is empty or over the cell cap. */
export type CanvasResult = Canvas | null

/** Flowchart and state diagrams: plain boxes, no extra content. */
export function layoutFlowchart(graph: Graph, limits: Limits): CanvasResult {
  const extras: NodeExtra[] = graph.nodes.map(() => ({ kind: 'plain' }))
  const canvas = layoutCanvas(graph, extras, limits)
  return canvas && orient(canvas, graph)
}

/** Class and ER diagrams: boxes divided into title / attribute / method rows. */
export function layoutClass(graph: Graph, limits: Limits): CanvasResult {
  const extras: NodeExtra[] = graph.nodes.map((node) => ({
    kind: 'compartments',
    sections: node.sections ?? [[node.label]],
  }))
  const canvas = layoutCanvas(graph, extras, limits)
  return canvas && orient(canvas, graph)
}

// -------------------------------------------------------------------- groups

/**
 * The overview of a grouped diagram: every top-level subgraph becomes one
 * node labelled with its title and member count in parentheses, edges between two
 * subgraphs (or a subgraph and a loose node) merge into one, and edges
 * inside a subgraph disappear. Multilevel drawing's coarsest level
 * (Walshaw), used when the full diagram is too wide for the space.
 */
function collapseGroups(graph: Graph): Graph {
  const out = new Graph(graph.dir)
  out.classDefs = graph.classDefs
  const topOf = (g: number | null): number | null => {
    let cur = g
    while (cur !== null && graph.groups[cur].parent !== null) cur = graph.groups[cur].parent
    return cur
  }
  const groupNode = new Map<number, number>()
  const members = new Map<number, number>()
  graph.nodeGroup.forEach((g) => {
    const t = topOf(g)
    if (t !== null) members.set(t, (members.get(t) ?? 0) + 1)
  })
  const nodeAt = new Map<number, number>()
  graph.nodes.forEach((node, v) => {
    const t = topOf(graph.nodeGroup[v])
    if (t === null) {
      if (graph.groups.some((g) => graph.index.get(g.id) === v)) return
      nodeAt.set(v, out.nodes.length)
      out.nodes.push(node)
      out.nodeGroup.push(null)
      return
    }
    let gn = groupNode.get(t)
    if (gn === undefined) {
      gn = out.nodes.length
      const count = members.get(t) ?? 0
      out.nodes.push({ label: `${graph.groups[t].label || graph.groups[t].id} (${count})`, shape: 'rect' })
      out.nodeGroup.push(null)
      groupNode.set(t, gn)
    }
    nodeAt.set(v, gn)
  })
  // A node whose id names a subgraph stands for it.
  graph.groups.forEach((g, gi) => {
    const v = graph.index.get(g.id)
    const t = topOf(gi)
    if (v !== undefined && t !== null && groupNode.has(t)) nodeAt.set(v, groupNode.get(t) as number)
  })
  const seen = new Set<string>()
  for (const e of graph.edges) {
    const a = nodeAt.get(e.from)
    const b = nodeAt.get(e.to)
    if (a === undefined || b === undefined || a === b) continue
    const key = `${a}>${b}`
    if (seen.has(key)) continue
    seen.add(key)
    out.edges.push({ ...e, from: a, to: b, label: null, cardFrom: undefined, cardTo: undefined })
  }
  return out
}

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
export function layoutGrouped(graph: Graph, limits: Limits): CanvasResult {
  if (limits.collapse) return layoutFlowchart(collapseGroups(graph), limits)
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

  const scope = buildScope(graph, null, scopeEdges, directNodes, keep, limits)
  return scope && orient(scope.canvas, graph)
}

/** A laid-out scope: its canvas and where every node inside it (at any depth) landed. */
interface Scope {
  canvas: Canvas
  anchors: Map<number, Anchor>
}

function buildScope(
  graph: Graph,
  scope: number | null,
  scopeEdges: Map<number | null, [ScopeItem, ScopeItem, number][]>,
  directNodes: Map<number | null, number[]>,
  keep: boolean[],
  limits: Limits,
): Scope | null {
  const items: ScopeItem[] = (directNodes.get(scope) ?? []).map((i) => ({ group: false, i }))
  const childGroups = graph.groups
    .map((_, gi) => gi)
    .filter((gi) => graph.groups[gi].parent === scope && keep[gi])
  items.push(...childGroups.map((i) => ({ group: true, i })))
  // In declaration order, a frame standing where its first member was
  // named: ranking breaks cycles from the first item, the author's entry.
  const firstIn = (gi: number): number => {
    let first = graph.nodes.length
    graph.nodeGroup.forEach((g, ni) => {
      for (let at: number | null = g; at !== null; at = graph.groups[at].parent) {
        if (at === gi) first = Math.min(first, ni)
      }
    })
    return first
  }
  const order = (item: ScopeItem): number => (item.group ? firstIn(item.i) : item.i)
  items.sort((a, b) => order(a) - order(b))

  if (items.length === 0) return { canvas: new Canvas(1, 1), anchors: new Map() }

  const nodeAt = new Map<number, number>()
  const groupAt = new Map<number, number>()
  const nodes: Node[] = []
  const extras: NodeExtra[] = []
  const subScopes = new Map<number, Scope>()
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
      const sub = buildScope(graph, item.i, scopeEdges, directNodes, keep, limits)
      if (sub === null) return null
      subScopes.set(nodes.length, sub)
      nodes.push({ label: graph.groups[item.i].label, shape: 'rect' })
      extras.push({ kind: 'frame', sub: sub.canvas })
    }
  }

  // An end standing for a frame remembers the inner node it really joins.
  const anchorOf = (item: ScopeItem, node: number): Anchor | undefined =>
    item.group ? subScopes.get(groupAt.get(item.i) as number)?.anchors.get(node) : undefined
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
      fromAnchor: anchorOf(f, e.from),
      toAnchor: anchorOf(t, e.to),
    }
    // Repeats of one inner-node pair ride the same cells; draw them once.
    // Top-down layout still ports at the frame (ponytail: anchors are wired
    // into placeLr only), so there every pair of frames is one edge.
    const ported = graph.dir === 'right' || graph.dir === 'left'
    const twin = (a: Edge, b: Edge): boolean =>
      a.from === b.from &&
      a.to === b.to &&
      (!ported || (a.fromAnchor?.node === b.fromAnchor?.node && a.toAnchor?.node === b.toAnchor?.node)) &&
      a.label === b.label &&
      a.headTo === b.headTo &&
      a.headFrom === b.headFrom &&
      a.line === b.line
    if ((f.group || t.group) && edges.some((x) => twin(x, collapsed))) continue
    edges.push(collapsed)
  }

  // Layout only reads nodes/edges/dir, so a bare Graph carrying those is enough.
  const synth = new Graph(graph.dir)
  synth.nodes = nodes
  synth.edges = edges
  const lay = layout(synth, extras, limits)
  if (lay === null) return null
  const canvas = paint(synth, extras, lay)

  const anchors = new Map<number, Anchor>()
  for (const item of items) {
    const li = (item.group ? groupAt : nodeAt).get(item.i) as number
    const p = lay.placed[li]
    if (!item.group) {
      anchors.set(item.i, { node: item.i, x: p.x, y: p.y, w: p.w, h: p.h })
      continue
    }
    const sub = subScopes.get(li) as Scope
    const [ox, oy] = frameOrigin(p, sub.canvas)
    for (const a of sub.anchors.values()) anchors.set(a.node, { ...a, x: a.x + ox, y: a.y + oy })
  }
  return { canvas, anchors }
}

/** Lay out and paint one scope. */
function layoutCanvas(graph: Graph, extras: NodeExtra[], limits: Limits): CanvasResult {
  const lay = layout(graph, extras, limits)
  return lay === null ? null : paint(graph, extras, lay)
}

// ------------------------------------------------------------------- drawing
