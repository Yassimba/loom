---
name: lineage
description: Trace the lineage of a command, function, method, or data object — the end-to-end alternating chain of data objects and the calls that transform them, each call carrying a one-line plain-words description. Use when the user asks for a lineage or how a value flows through a command end to end.
---

# Lineage

Given one symbol — a CLI command, a function, a method, or a data object — produce the full lineage it belongs to: entry point to terminal effect, as a strict **alternation** of data objects and the calls that transform them. Two renders every time: a text chain in the reply and an interactive diagram page.

## 1. Anchor

Resolve the symbol in the current codebase. A mid-chain symbol is never the whole story: walk **up** to the entry point that reaches it (CLI command, HTTP route, event handler, cron) and **down** to the terminal effect (stdout, response, file, exit code). The lineage always runs entry to exit; the asked-for symbol is just where the digging started.

Done when: entry point and terminal effect are both located in source, with file paths.

## 2. Trace

**Graph first.** When `graphify-out/graph.json` exists, take the skeleton from the graphify CLI instead of reading files:

```bash
graphify query "How does <entry> work end to end?"   # the whole skeleton in one answer
graphify explain "<symbol>"                          # one node: every edge, callers and callees
graphify path "<a>" "<b>"                            # shortest path between two symbols
```

Answers carry file:line and graphify's extracted rationales — raw material for wtf lines. Source is then read only to verify the chosen path's hops and signatures, never to discover them.

**Freshness gate** — the graph is only as current as its manifest, so check before trusting the skeleton:

```bash
python3 -c "import json,os; m=json.load(open('graphify-out/manifest.json')); d=[p for p,e in m.items() if not os.path.exists(p) or os.path.getmtime(p)>e['mtime']+1]; print(len(d),'of',len(m),'files drifted'); [print(' ',p) for p in d[:10]]"
```

Drift on the traced flow's files means the graph lies about that flow — run `/graphify . --update` first (incremental; code re-extracts without an LLM). Drift elsewhere is tolerable: note it and continue.

Without a graph, walk the chain hop by hop against the real code — LSP `goToDefinition`/`outgoingCalls` where it beats grep, Read for signatures. Every hop is one of exactly two kinds:

- **data object** — a value that travels: type name + the fields that matter
- **call** — a transformation: real name, parameters, and return type exactly as written in the signature; a method also names the class it is bound to (`TreeSitterParser.parse`). A `→ None` return is information, not an omission — it says the call speaks through a side channel (sink, console), so name that channel in the wtf line.

The chain alternates. Two adjacent nodes of the same kind mean a missing hop — go find it in the source. A dispatch, claim check, or error fork becomes a decision point with one edge per outcome; a loop over a collection becomes one labeled per-item region, traced once. A call taking several data objects gets **fan-in**: each input drawn as its own node with a converging edge, each carrying its own provenance branch — bundling independent inputs into one "wiring" blob is a missing-hop smell.

Done when: every node names a symbol verified in source and the alternation holds unbroken from entry to exit.

## 3. The wtf line

Every call gets one line under its signature: what it actually does, in the plainest words that are still true. Active voice, concrete, at most ~10 words — "walks the tree, reads every YAML it can, never raises", never "performs filesystem traversal and document acquisition". Apply the writing-clearly-and-concisely skill's rules: omit needless words, positive form, specifics over abstractions.

Done when: every call node carries its line and none reads like documentation.

## 4. Render twice

**Text chain**, printed in the reply — fenced, vertical, annotations right of each hop:

```
$ entry command                          cli/app.py
        ▼
handler(args, flags) → None              checks your flags make sense
        ▼
DataObject  field · field                where the value now lives
        ▼
Class.transform(input, deps) → Output    what it does in plain words
        ▼
terminal effect (stdout, exit code)
```

**Diagram** — one `flowchart TD` in a `.mmd`, rendered by the bundled viewer. Styling contract, identical every run so lineages stay comparable:

- data objects: cylinders `[("Name<br/>field · field")]` — `classDef data fill:#e3f2fd,stroke:#1565c0,stroke-width:2px,color:#0d47a1`
- free functions: sharp boxes `["fn(params) → Return<br/><i>wtf line</i>"]` — `classDef fn fill:#f5f5f5,stroke:#616161,color:#212121`
- instance methods (bound to a live object): rounded violet boxes `("Class.method(params) → Return<br/><i>wtf line</i>")` — `classDef method fill:#ede7f6,stroke:#5e35b1,stroke-width:2px,color:#311b92`
- classmethods (bound to the type; usually a named constructor): deeper-violet stadiums `(["Class.method(params) → Return<br/><i>wtf line</i>"])` — `classDef cmethod fill:#d1c4e9,stroke:#4527a0,stroke-width:2px,color:#311b92`
- staticmethods (namespaced on the class, bound to nothing): rounded violet, dashed — `classDef smethod fill:#ede7f6,stroke:#5e35b1,stroke-width:2px,stroke-dasharray: 4 4,color:#311b92`
- every method label names the owning class, whatever the binding
- dead ends (skips, rejections): `classDef stop fill:#fce4ec,stroke:#ad1457,color:#880e4f`
- decisions: diamonds `{"question?"}` — `classDef decision fill:#fffde7,stroke:#f9a825,color:#5d4037` · per-item loops: one `subgraph`
- explicit `color:` on every classDef — dark-theme viewers otherwise wash the labels out

Save as `lineage-<slug>.mmd` beside the repo's explanation docs (`ai-docs/explanations/` or equivalent) when the repo has such a tree, else in the session scratchpad. Build the page with the bundled viewer:

```bash
<skill-dir>/scripts/render.sh -t "<symbol>" [-d lineage-<slug>.details.json] lineage-<slug>.mmd
```

The page adds pan/zoom, search, and platform-native keyboard navigation: Primary+↑/↓ follows arrows, Primary+←/→ switches branches from any depth, and Shift+Primary+←/→ crosses diff worlds. Flowchart nodes, class nodes, and sequence messages are navigable. With `-d`, author only inspectable items per [details.md](./details.md); plain lineage entries use bare ids and omit verdicts. Show the page inline when possible; use `--open` only when the user asked to open it.

Browser validation is optional and must never launch or reuse the user's installed desktop browser automatically. Run `render.sh --validate-browser` only when `LINEAGE_HEADLESS_BROWSER` already names a dedicated headless executable supplied for automation. Never set it to Google Chrome, Chromium, Edge, or another desktop application on the user's behalf. If validation is unavailable, report the HTML as unvalidated and stop. Do not fall back to `mmdc`, Puppeteer, Playwright, `open`, or another browser launcher unless the user explicitly asks for visual validation.

Done when: the reply carries the text chain and the HTML exists. State whether browser validation ran; an unvalidated artifact is acceptable by default.
