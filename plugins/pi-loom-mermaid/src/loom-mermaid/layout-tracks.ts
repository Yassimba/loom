/**
 * Track allocation and cross-axis packing for layered graphs.
 */
import type { Edge, Graph, LineKind } from './graph.ts'
import { brandesKoepf } from './placement.ts'
import { edgeText, half, type Placed } from './layout-geom.ts'
import type { Layered } from './layout-rank.ts'

/** Cross-axis centres for every layered node, real and virtual. */
export function assignPositions(
  layered: Layered,
  size: number[],
  sep: number,
  pad: (node: number) => number = () => 0,
  offset: (v: number) => number = () => 0,
  padLeft: (node: number) => number = () => 0,
): number[] {
  const n = size.length
  const all = [...size]
  while (all.length < layered.up.length) all.push(1)
  // `pad(v)` reserves cells right of `v` for a label: a real node's arrival
  // labels when a chain follows it, or a chain node's own edge label;
  // `padLeft(v)` the same on its left.
  // Two real nodes keep `sep`, or more when a label — the left one's two
  // cells right of its centre, the right one's ending two cells left of
  // its centre — would run into the other box or label.
  const sepOf = (left: number, right: number): number => {
    if (left >= n || right >= n) return 1 + pad(left) + padLeft(right)
    const [r, l] = [size[left] - half(size[left]), half(size[right])]
    const [a, b] = [pad(left), padLeft(right)]
    // Label to label: halves as the compaction measures them, unrounded.
    const both = a > 0 && b > 0 ? a + b + 2 - size[left] / 2 - size[right] / 2 : 0
    return Math.max(sep, a > 0 ? a + 2 - r : 0, b > 0 ? b + 2 - l : 0, both)
  }
  return brandesKoepf(layered, all, sepOf, n, offset)
}

// ------------------------------------------------------------------- tracks

/**
 * A span competing for a track: the covered coordinate range, the arms
 * that reach it from either side, and its edge. In a band between ranks,
 * `up` arms come from the earlier rank and `down` arms lead on to the
 * later one (in a lane strip both arms are `up`).
 */
export interface TrackSpan {
  start: number
  end: number
  from: number
  to: number
  edge: number
  up: number[]
  down: number[]
  /** A labelled lane refuses endpoint sharing in `packTracks`: the label
   * would appear to cover every edge merged onto the row. */
  /** Edge text; runs share a trunk only when it reads the same on each. */
  label?: string | null
  /** Spans with one key run as one trunk: every edge between two frames shares a bus. */
  bundle?: string
  /** Line style; runs of different styles never share a trunk. */
  line?: LineKind
}

/** Spans merged onto one track because they share an endpoint. */
interface Hyper {
  members: TrackSpan[]
  start: number
  end: number
  up: number[]
  down: number[]
}

/**
 * Order spans onto parallel tracks, nearest the earlier rank first, so
 * that arms cross as few other spans' runs as possible (Sander's segment
 * ordering, as in ELK's orthogonal router): spans that share an endpoint
 * merge into one run — edges fanning out of one node draw one `┴` origin
 * rather than a stack of them — then every two runs that overlap are
 * compared both ways, the cheaper order becomes a dependency, cycles are
 * broken greedily, and a run's track is its longest dependency path. Runs
 * two cells apart share a track.
 */
export function assignTracks(spans: TrackSpan[]): { assigned: [number, number][]; count: number } {
  const hypers = mergeShared(spans)
  const n = hypers.length
  const overlaps = (a: Hyper, b: Hyper): boolean => a.start <= b.end + 1 && b.start <= a.end + 1
  /** Crossings when `a` runs on the track nearer the earlier rank than `b`. */
  const crossings = (a: Hyper, b: Hyper): number =>
    a.down.filter((x) => b.start < x && x < b.end).length +
    b.up.filter((x) => a.start < x && x < a.end).length
  const weight: number[][] = Array.from({ length: n }, () => new Array<number>(n).fill(-1))
  for (let i = 0; i < n; i++) {
    for (let j = i + 1; j < n; j++) {
      if (!overlaps(hypers[i], hypers[j])) continue
      const ij = crossings(hypers[i], hypers[j])
      const ji = crossings(hypers[j], hypers[i])
      // A run arriving on a row where another run leaves goes outside it:
      // the fork off that row must come before the join, or the joined
      // edge reads as forking too (`B ─●─●─▶ Y` with A's join first says
      // A also reaches Z).
      const forkFirst = (fork: Hyper, join: Hyper): boolean => fork.up.some((y) => join.down.includes(y))
      if (forkFirst(hypers[i], hypers[j]) && !forkFirst(hypers[j], hypers[i])) weight[i][j] = 1
      else if (forkFirst(hypers[j], hypers[i]) && !forkFirst(hypers[i], hypers[j])) weight[j][i] = 1
      // Equal: keep the earlier-starting run nearer, the packing order.
      else if (ij < ji || (ij === ji && hypers[i].start <= hypers[j].start)) weight[i][j] = ji - ij
      else weight[j][i] = ij - ji
    }
  }
  const order = greedyAcyclic(weight)
  const track = new Array<number>(n).fill(0)
  for (const v of order) {
    for (let u = 0; u < n; u++) {
      if (weight[u][v] >= 0 && track[u] + 1 > track[v]) track[v] = track[u] + 1
    }
  }
  const assigned: [number, number][] = []
  hypers.forEach((h, i) => {
    for (const m of h.members) assigned.push([m.edge, track[i]])
  })
  return { assigned, count: n === 0 ? 0 : Math.max(...track) + 1 }
}

/**
 * Pack lane spans into as few tracks as possible, shortest first: a span
 * contained in another takes the inner track, so exits and entries at rows
 * the inner lane never reaches cross nothing. Lanes trade crossings for
 * height, where `assignTracks`' dependency chains would cost a track each.
 */
export function packTracks(spans: TrackSpan[], weak: Set<number> = new Set()): { assigned: [number, number][]; count: number } {
  const sorted = [...spans].sort(
    (a, b) =>
      Number(weak.has(a.edge)) - Number(weak.has(b.edge)) ||
      a.end - a.start - (b.end - b.start) ||
      a.start - b.start ||
      a.end - b.end ||
      a.from - b.from ||
      a.to - b.to ||
      a.edge - b.edge,
  )
  const tracks: TrackSpan[][] = []
  const assigned: [number, number][] = []
  for (const span of sorted) {
    let slot = tracks.findIndex((members) =>
      members.every(
        (m) =>
          m.end + 2 <= span.start ||
          span.end + 2 <= m.start ||
          ((m.from === span.from || m.to === span.to) && m.label === span.label),
      ),
    )
    if (slot === -1) {
      tracks.push([])
      slot = tracks.length - 1
    }
    tracks[slot].push(span)
    assigned.push([span.edge, slot])
  }
  return { assigned, count: tracks.length }
}

function mergeShared(spans: TrackSpan[]): Hyper[] {
  const sorted = [...spans].sort(
    (a, b) => a.start - b.start || a.end - b.end || a.from - b.from || a.to - b.to || a.edge - b.edge,
  )
  // One arm per coordinate: edges sharing a port share the arm.
  const build = (members: TrackSpan[]): Hyper => ({
    members,
    start: Math.min(...members.map((m) => m.start)),
    end: Math.max(...members.map((m) => m.end)),
    up: [...new Set(members.flatMap((m) => m.up))],
    down: [...new Set(members.flatMap((m) => m.down))],
  })
  const hypers: Hyper[] = []
  for (const span of sorted) {
    const k = hypers.findIndex((h) => {
      const sameFrom = h.members.every((m) => m.from === span.from && m.up[0] === span.up[0])
      const sameTo = h.members.every((m) => m.to === span.to && m.down[0] === span.down[0])
      const bundled = span.bundle !== undefined && h.members.some((m) => m.bundle === span.bundle)
      // A fan (one source or one target) shares its trunk across line
      // styles: each arm keeps its own stroke, and the canvas paints the
      // shared run solid. Two private edges into one target from the same
      // side cross unless they share that run (proofs/PrivateFanIn.lean).
      // A bundle keeps one style: its trunk stands for every pair.
      const sameLine = h.members.every((m) => m.line === span.line)
      return sameFrom || sameTo || (bundled && sameLine)
    })
    if (k === -1) hypers.push(build([span]))
    else hypers[k] = build([...hypers[k].members, span])
  }
  // A trunk is correct iff every source on it reaches every target on it
  // (proofs/TrunkCorrect.lean); one-source and one-target fans are the
  // trivial cases. Two trunks may merge iff every cross pair is an edge
  // (`merge_iff` there), which `complete` checks. Runs whose members join
  // every source of the one to every target of the other form one bundle
  // (Newbery's edge concentration), so a full fan-out-into-fan-in draws
  // one trunk with one head per target. Labelled edges stay apart, since
  // a label on the trunk would name every edge.
  const srcs = (h: Hyper): Set<number> => new Set(h.members.map((m) => m.from))
  const dsts = (h: Hyper): Set<number> => new Set(h.members.map((m) => m.to))
  const complete = (a: Hyper, b: Hyper): boolean => {
    const S = new Set([...srcs(a), ...srcs(b)])
    const T = new Set([...dsts(a), ...dsts(b)])
    const have = new Set([...a.members, ...b.members].map((m) => `${m.from}>${m.to}`))
    if ([...a.members, ...b.members].some((m) => m.label !== a.members[0].label || m.line !== a.members[0].line)) return false
    for (const x of S) for (const y of T) if (!have.has(`${x}>${y}`)) return false
    return true
  }
  // Partial bicliques: two fans that share two or more targets (or
  // sources) split off the shared part as one trunk, so `a -> {x,y,z}`
  // and `b -> {x,y}` draw `{a,b} -> {x,y}` plus a lone `a -> z`. The
  // lone edge would otherwise cost a crossing the trunk avoids.
  // ponytail: greedy pairwise; a maximal-biclique search if it ever matters.
  for (let i = 0; i < hypers.length; i++) {
    for (let j = hypers.length - 1; j > i; j--) {
      const a = hypers[i]
      const b = hypers[j]
      const [ma, mb] = [a.members[0], b.members[0]]
      if (ma.line !== mb.line || ma.label !== mb.label || complete(a, b)) continue
      for (const side of ['to', 'from'] as const) {
        const other = side === 'to' ? 'from' : 'to'
        const key = (m: TrackSpan): number => m[side]
        if (new Set(a.members.map((m) => m[other])).size !== 1 || new Set(b.members.map((m) => m[other])).size !== 1) continue
        // A member only joins the trunk when its twin has the same style.
        const twin = (m: TrackSpan, list: TrackSpan[]): boolean =>
          list.some((n) => key(n) === key(m) && n.line === m.line && n.label === m.label)
        const shared = new Set(a.members.filter((m) => twin(m, b.members)).map(key))
        if (shared.size < 2) continue
        const inA = a.members.filter((m) => shared.has(key(m)))
        const inB = b.members.filter((m) => shared.has(key(m)))
        const restA = a.members.filter((m) => !shared.has(key(m)))
        const restB = b.members.filter((m) => !shared.has(key(m)))
        hypers[i] = build([...inA, ...inB])
        hypers.splice(j, 1)
        if (restA.length) hypers.push(build(restA))
        if (restB.length) hypers.push(build(restB))
        break
      }
    }
  }
  // Merge complete pairs to a fixpoint: joining two trunks can complete a
  // third against the union, so one pass leaves needless dots behind.
  let merged = true
  while (merged) {
    merged = false
    for (let i = 0; i < hypers.length; i++) {
      for (let j = hypers.length - 1; j > i; j--) {
        if (!complete(hypers[i], hypers[j])) continue
        hypers[i] = build([...hypers[i].members, ...hypers[j].members])
        hypers.splice(j, 1)
        merged = true
      }
    }
  }
  return hypers
}

/**
 * Eades–Lin–Smyth greedy cycle removal on a weighted dependency matrix:
 * returns a vertex order; dependencies pointing backwards in it are
 * dropped (set to -1). Sinks go last, sources first, else the vertex with
 * the largest outgoing-minus-incoming weight goes first.
 */
function greedyAcyclic(weight: number[][]): number[] {
  const n = weight.length
  const alive = new Array<boolean>(n).fill(true)
  const head: number[] = []
  const tail: number[] = []
  const sum = (v: number, incoming: boolean): number => {
    let total = 0
    for (let u = 0; u < n; u++) {
      const w = incoming ? weight[u][v] : weight[v][u]
      if (alive[u] && w >= 0) total += w + 1
    }
    return total
  }
  let left = n
  while (left > 0) {
    let progressed = false
    for (let v = 0; v < n; v++) {
      if (!alive[v]) continue
      if (sum(v, false) === 0) {
        tail.push(v)
        alive[v] = false
        left--
        progressed = true
      } else if (sum(v, true) === 0) {
        head.push(v)
        alive[v] = false
        left--
        progressed = true
      }
    }
    if (progressed || left === 0) continue
    let best = -1
    let bestScore = Number.NEGATIVE_INFINITY
    for (let v = 0; v < n; v++) {
      if (!alive[v]) continue
      const score = sum(v, false) - sum(v, true)
      if (score > bestScore) {
        bestScore = score
        best = v
      }
    }
    head.push(best)
    alive[best] = false
    left--
  }
  const order = [...head, ...tail.reverse()]
  const pos = new Array<number>(n).fill(0)
  order.forEach((v, i) => (pos[v] = i))
  for (let u = 0; u < n; u++) for (let v = 0; v < n; v++) if (weight[u][v] >= 0 && pos[u] > pos[v]) weight[u][v] = -1
  return order
}

/**
 * Edges in a complete bipartite subgraph between adjacent ranks, keyed by
 * that biclique, so `{a,b} -> {x,y}` rides one trunk (a confluent bundle:
 * unambiguous, since every source really does reach every target) and a
 * lone `a -> z` beside it takes its own track. Greedy over source pairs:
 * two unlabelled sources of one line style sharing two or more targets
 * form the seed, further sources join while they reach every target.
 */
export function bicliqueKeys(graph: Graph, ranks: number[]): Map<number, string> {
  const out = new Map<number, string>()
  const plain = (e: Edge): boolean => e.label === null && e.from !== e.to
  const fan = new Map<number, Map<number, number>>()
  graph.edges.forEach((e, i) => {
    if (!plain(e) || ranks[e.to] !== ranks[e.from] + 1) return
    if (!fan.has(e.from)) fan.set(e.from, new Map())
    fan.get(e.from)?.set(e.to, i)
  })
  const sources = [...fan.keys()]
  for (const a of sources) {
    for (const b of sources) {
      if (b <= a) continue
      const fa = fan.get(a) as Map<number, number>
      const fb = fan.get(b) as Map<number, number>
      const line = graph.edges[[...fa.values()][0]].line
      const targets = [...fa.keys()].filter(
        (t) => fb.has(t) && !out.has(fa.get(t) as number) && !out.has(fb.get(t) as number) &&
          graph.edges[fa.get(t) as number].line === line && graph.edges[fb.get(t) as number].line === line,
      )
      if (targets.length < 2) continue
      const members = [a, b]
      for (const c of sources) {
        const fc = fan.get(c) as Map<number, number>
        if (members.includes(c) || !targets.every((t) => fc.has(t) && !out.has(fc.get(t) as number) && graph.edges[fc.get(t) as number].line === line)) continue
        members.push(c)
      }
      const key = `${members.join(',')}>${targets.join(',')}`
      for (const m of members) for (const t of targets) {
        const i = fan.get(m)?.get(t) as number
        out.set(i, key)
      }
    }
  }
  return out
}

/** Forward edges crossing the band between rank `r` and `r + 1` that must
 * jog sideways, so need a bus row. */
export function busSpans(
  graph: Graph,
  ranks: number[],
  centers: number[],
  r: number,
  exact: boolean,
  entry: (edge: number) => number = (i) => centers[graph.edges[i].to],
  exit: (edge: number) => number = (i) => centers[graph.edges[i].from],
  bundle: (edge: number) => string | undefined = () => undefined,
): TrackSpan[] {
  const out: TrackSpan[] = []
  graph.edges.forEach((e, i) => {
    const jogs =
      bundle(i) !== undefined ||
      (exact ? exit(i) !== entry(i) : Math.abs(exit(i) - entry(i)) > 1)
    if (e.from !== e.to && ranks[e.to] === ranks[e.from] + 1 && ranks[e.from] === r && jogs) {
      const arrive = entry(i)
      out.push({
        start: Math.min(exit(i), arrive),
        end: Math.max(exit(i), arrive),
        from: e.from,
        to: e.to,
        edge: i,
        up: [exit(i)],
        down: [arrive],
        label: edgeText(e),
        line: e.line,
        bundle: bundle(i),
      })
    }
  })
  return out
}

/**
 * Edges whose ends are already connected the long way round, so the line
 * states a reachability the drawing shows anyway (`A -> C` beside
 * `A -> B -> C`). The Path Based Framework treats these as the cheapest
 * ink on the page — it bundles them off to the side of a path, and its
 * companion work drops some entirely, because reachability survives
 * without them (Ortali and Tollis, JGAA 2023). Here they keep their line
 * but lose every tie: a redundant run takes the outermost lane, so an
 * informative one never has to reach around it.
 */
export function transitiveEdges(graph: Graph): Set<number> {
  const out: number[][] = graph.nodes.map(() => [])
  graph.edges.forEach((e, i) => {
    if (e.from !== e.to) out[e.from].push(i)
  })
  const weak = new Set<number>()
  graph.edges.forEach((e, i) => {
    if (e.from === e.to) return
    // Depth-first from the source, never taking this edge: reaching the
    // target means the drawing already says so.
    const seen = new Set<number>([e.from])
    const stack = [e.from]
    while (stack.length > 0) {
      for (const k of out[stack.pop() as number]) {
        const next = graph.edges[k].to
        if (k === i || seen.has(next)) continue
        if (next === e.to) {
          weak.add(i)
          return
        }
        seen.add(next)
        stack.push(next)
      }
    }
  })
  return weak
}

/** Left-to-right edges skipping a rank or running backwards that go around
 * in a lane below the diagram. */
export function laneSpans(graph: Graph, ranks: number[], ends: (i: number) => [Placed, Placed]): TrackSpan[] {
  const out: TrackSpan[] = []
  graph.edges.forEach((e, i) => {
    if (e.from === e.to || ranks[e.to] === ranks[e.from] + 1) return
    const [pf, pt] = ends(i)
    const a = Math.min(pf.cx, pt.cx)
    const b = Math.max(pf.cx, pt.cx)
    out.push({
      start: a,
      end: b,
      from: e.from,
      to: e.to,
      edge: i,
      up: [pf.cx, pt.cx],
      down: [],
      label: edgeText(e),
    })
  })
  return out
}

// ----------------------------------------------------------------- placement

/** One sideways jog of an interior skip route, competing for a bus track. */
export interface ChainJog extends TrackSpan {
  band: number
  /** Cross-axis coordinate the edge continues along after the jog. */
  at: number
}

/**
 * The jogs an interior edge makes following its virtual chain: exit
 * coordinate to the first chain coordinate, between chain nodes where they
 * differ, and from the last one to the entry coordinate. Edges `exit`
 * returns `null` for take no part (they stay on a lane). A back edge walks
 * its bands upward.
 */
export function chainJogs(
  graph: Graph,
  ranks: number[],
  layered: Layered,
  centers: number[],
  ends: (e: Edge, i: number) => { exit: number; entry: number } | null,
): ChainJog[] {
  const jogs: ChainJog[] = []
  graph.edges.forEach((e, i) => {
    const at = ends(e, i)
    if (at === null) return
    const chain = layered.chains[i]
    const stops = [at.exit, ...chain.map((v) => centers[v]), at.entry]
    const ids = [e.from, ...chain, e.to]
    const upward = ranks[e.to] < ranks[e.from]
    for (let k = 0; k + 1 < stops.length; k++) {
      if (stops[k] === stops[k + 1]) continue
      jogs.push({
        band: upward ? ranks[e.from] - 1 - k : ranks[e.from] + k,
        at: stops[k + 1],
        start: Math.min(stops[k], stops[k + 1]),
        end: Math.max(stops[k], stops[k + 1]),
        from: ids[k],
        to: ids[k + 1],
        edge: i,
        up: [upward ? stops[k + 1] : stops[k]],
        down: [upward ? stops[k] : stops[k + 1]],
      })
    }
  })
  return jogs
}

/** Per edge, its jogs as route waypoints once bus coordinates are known. */
export function skipRoutes(
  graph: Graph,
  jogs: ChainJog[],
  busOf: (j: ChainJog) => number,
): { bus: number; at: number }[][] {
  const routes: { bus: number; at: number }[][] = graph.edges.map(() => [])
  for (const j of jogs) routes[j.edge].push({ bus: busOf(j), at: j.at })
  return routes
}

/**
 * A chain column that coincides with a port column of the rank above or
 * below would share cells with that port's vertical inside the band (a
 * forward exit at the centre of the box above; a back exit beside centre
 * in the box below, which climbs past the forward tracks). Nudge
 * such a chain node by a cell where the gaps to its neighbours allow.
 */
export function clearPorts(
  graph: Graph,
  layered: Layered,
  centers: number[],
  size: number[],
  backExit: (node: number) => number[],
): void {
  const n = graph.nodes.length
  const ends = new Map<number, number[]>()
  graph.edges.forEach((e, i) => {
    for (const v of layered.chains[i]) ends.set(v, [...(ends.get(v) ?? []), e.from, e.to])
  })
  layered.layers.forEach((row, r) => {
    /** Port column → the node owning it; a chain's own endpoints are no conflict. */
    const ports = new Map<number, number[]>()
    const claim = (col: number, u: number): void => {
      const owners = ports.get(col)
      if (owners) owners.push(u)
      else ports.set(col, [u])
    }
    for (const u of layered.layers[r - 1] ?? []) if (u < n) claim(centers[u], u)
    for (const u of layered.layers[r + 1] ?? []) {
      if (u < n) for (const col of backExit(u)) claim(col, u)
    }
    row.forEach((v, i) => {
      if (v < n) return
      const own: number[] = ends.get(v) ?? []
      const blocked = (col: number): boolean =>
        (ports.get(col) ?? []).some((u) => !own.includes(u))
      if (!blocked(centers[v])) return
      const left = row[i - 1]
      const right = row[i + 1]
      const lo = left === undefined ? 0 : centers[left] + Math.ceil(size[left] / 2) + 1
      const hi = right === undefined ? Number.MAX_SAFE_INTEGER : centers[right] - Math.ceil(size[right] / 2) - 1
      for (const d of [1, -1, 2, -2]) {
        const c = centers[v] + d
        if (c >= lo && c <= hi && !blocked(c)) {
          centers[v] = c
          return
        }
      }
    })
  })
}

