---
name: explain-code-flow
description: "Architecture walkthrough of one feature: layered diagrams (overview → structure → runtime data spine → zooms) and anchored prose, opened in Plannotator for annotation. Use for a big-picture explanation or end-to-end flow of a feature, ER/class/sequence diagrams of it, or a colored diff of how its flow changed between two revisions."
---

# Explain Code Flow

Explain one feature from system shape to runtime values. Every factual node and edge has source evidence; every figure proves one fact.

Minimize model round trips: use one fresh evidence worker, then let the parent compile every artifact from one authored `explanation.py`. After the brief returns, read the brief, compact drawing API, bundle example, and invariants together; emit the bundle in one turn; run its compiler once; inspect all PNGs in one parallel read; consolidate repairs into one turn and recompile. Failures may add cycles—never skip quality work. Do not spawn figure, prose, planner, or validator workers, and set no turn or usage budget.

**Diff mode:** when the user names a range, branch, PR, or change, load [`references/diagram-diff.md`](references/diagram-diff.md) at step 1. Pin `from` and `to` (`to` defaults to the working tree) and add a colored diff for each affected figure.

## 1. Set scope

Name the feature, live entry, source area, and final result or effect. Verify production code composes and reaches it; otherwise the composition gap is part of the explanation.

Done: one explicit boundary from live entry to result, plus pinned revisions in diff mode.

## 2. Map evidence

If `.codegraph/` exists, query it once for the entry, central types, final-result function, and call graph; use exact reads only for elided lines. Otherwise spawn one **fresh-context worker** with [`references/repository-evidence.md`](references/repository-evidence.md), [`references/content-brief-by-type.md`](references/content-brief-by-type.md), the feature boundary, repo root, and output path. It writes `ai-docs/explanations/<feature>/brief.md`, appends the justified figure list from step 4, and runs `check-anchors.py`. In diff mode, include the change contract from `diagram-diff.md`. Wait for it; the parent does not inspect production files or repeat its mapping.

The brief contains the boundary, anchored entry chain, figure-worthy types/functions, runtime boundaries, externals, values, state changes, and composition gaps. Field inventories and branch trivia belong in the walkthrough. It is the only repository evidence drawing workers receive.

Done: the map reaches the effect, every named fact exists in source, and every anchor came from command output.

## 3. Verify

Do not repeat repository discovery. Run:

```bash
python3 scripts/check-anchors.py <repo-root> brief.md
```

Scan each printed source line once. If it does not support the claim, replace the anchor from fresh `grep -n`; unresolved edges become labelled assumptions.

Done: exit 0 and every printed line supports its claim.

## 4. Choose figures

Work down the ladder. A rung qualifies by its predicate, then survives only when removing it would leave a specific reader misunderstanding. Skip redundancy; figure count is evidence-driven, never an optimization target. Load [`references/content-brief-by-type.md`](references/content-brief-by-type.md) and [`references/authoring-invariants.md`](references/authoring-invariants.md).

| Rung | Fact | Type | Predicate |
| --- | --- | --- | --- |
| 1. Overview | entry, components, externals, result, gap | Architecture | always |
| 2. Layers | layers and dependency direction | Layer stack | crosses layers |
| 3. Structure | central types and relationships | ER for entities; schema for tables; UML for protocols/inheritance | ≥3 central types |
| 4. Spine | value in → transformations/state/I/O → value out | Sequence when order matters; Data flow when custody/shape matters | always |
| 5. Lifecycle | guarded states and terminal outcomes | State machine | owns state/status |
| 6. Zoom | one hidden decision tree, loop, or dense stage | Flowchart, Sequence, or Data flow | spine stage hides complexity |

Use another type only when it uniquely proves a fact, such as Dependency for fan-in, Swimlane for process handoffs, or Deployment for hosts.

Append the figure list to `brief.md`: filename, type, nodes, and one fact proved; include diff variants.

## 5. Draw and export

Figures are Python scripts over [`scripts/draw.py`](scripts/draw.py). A missing or `profile: default` `.diagram-design` marker uses its palette; otherwise resolve diagram-design profile tokens and override the kit constants.

The parent reads exactly `brief.md`, `scripts/draw.py`, `scripts/example-bundle.py`, and `references/authoring-invariants.md`; it reads neither production files, `_draw_impl.py`, `bundle.py`, nor diagram-design. Plan explicit coordinates and routes for every figure. Write one `explanation.py` using the example's `BUNDLE`/`op` format; figure list order is paint order.

Compile everything with one command:

```bash
python3 <skill>/scripts/bundle.py --repo <repo-root> ai-docs/explanations/<feature>/explanation.py
```

The compiler validates primitive calls and high-confidence geometry, renders exact declared artifacts, rasterizes, checks anchors, and builds inlined HTML; it never lays out or repairs a figure. Inspect every PNG in one parallel read. Consolidate fixes in `explanation.py`, then rerun the compiler once.

Done: every planned figure has `.html`, `.svg`, a viewed PNG, exact artifact/embed sets, and passing checks; `explanation.py` is the single reviewed source.

## 6. Write walkthrough

The parent supplies title, context, sections, facts, and result inside `explanation.py`; the compiler writes `walkthrough.md`. The parent reads no production files. Use the `writing-clearly-and-concisely` register. Word bands by relevant files: 1–3 → 150–300; 4–10 → 300–600; 11+ → 500–900.

Order: **Context** (entry, output, scope; ≤3 lines), one section per drawn rung, optional **What changed**, then **Result**. Each rung has one bold claim, its SVG embed, and ≤5 anchored facts. Structure lists 3–7 central types grouped by layer when large. Spine is a numbered hop list: function, value in, value out, state/side effect. Separate production call sites from tests when reuse matters.

Done: Overview and Spine form one path, each flow appears once, every figure is embedded once, and structural claims are anchored.

## 7. Validate and annotate

```bash
python3 scripts/check-anchors.py <repo-root> walkthrough.md --quiet
python3 scripts/build-html.py walkthrough.md
```

The bundle compiler runs these commands and rejects missing, stale, duplicate, or unembedded artifacts. Consolidate failures into one `explanation.py` repair turn where possible, then rerun it. Then run `plannotator annotate walkthrough.html` in the background without a timeout; it blocks until submission. Address returned annotations and rebuild after changes. If closed, stop.

In chat, return the Result paragraph and walkthrough path.