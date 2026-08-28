---
name: explain-code-flow
description: "Architecture walkthrough of one feature: layered diagrams (overview → structure → runtime data spine → zooms) and anchored prose, opened in Plannotator for annotation. Use for a big-picture explanation or end-to-end flow of a feature, ER/class/sequence diagrams of it, or a colored diff of how its flow changed between two revisions."
---

# Explain Code Flow

Explain one feature from system shape down to runtime detail. Every drawn edge has source evidence; every figure proves one fact; the reader annotates the result in Plannotator.

**Diff mode** applies when the user names a git range, branch, or PR, or asks what changed: draw the feature as it is now, then a colored diff of each figure the change touches. Diff mode's rules live in [`references/diagram-diff.md`](references/diagram-diff.md); load it at step 1.

## 1. Set the scope

Find the feature, its live entry point, and its final result or effect. Confirm production code assembles and reaches it; when nothing does, the walkthrough shows that composition gap. In diff mode, pin `from` and `to` (`to` defaults to the working tree).

Done when the feature boundary, the source area, and any revisions are pinned.

## 2. Map the evidence

When the repo has a `.codegraph/` index, query it first, once, for the call graph and the signatures: name the entry, the central types, and the final-result function. Its output is capped, so it elides lines inside long files; read what it leaves open with exact `sed -n 'a,bp'` ranges rather than a second wide query. Otherwise spawn one relentless Explore worker with [`references/repository-evidence.md`](references/repository-evidence.md): its inspection order and its Deliverables section are the worker's whole contract. In diff mode, also have it compute the change as `diagram-diff.md` describes.

An anchor is only ever copied from a `grep -n` or `sed -n` result line, never typed from memory of a range: `grep -n "fn confirm_review" src/wizard/state.rs` → `state.rs:552`. Write the map to `ai-docs/explanations/<feature-slug>/brief.md` (evidence first, figure list appended in step 4); it is the drawing worker's whole input.

Done when the map reaches from the live entry to the final result, every entity and state named appears in source, every anchor came from a grep or sed line, and any composition gap is identified.

## 3. Verify

Do not reread what the worker read. Run `python3 scripts/check-anchors.py <repo-root> brief.md`: it resolves every `file:line` and prints the source line beside it. Scan the list once; an anchor whose printed line does not carry the claimed symbol is drift, fix it from a fresh `grep -n`. Open a file only where the map is contradictory or an edge lacks an anchor; draw that edge only once anchored, otherwise label it an assumption.

Done when the check exits 0 and every printed line supports its claim.

## 4. Choose the figures

Choose from the top of this ladder down; the type per rung is fixed here, so diagram-design's SKILL.md is not loaded. Each rung is one figure proving one fact, inside the budget in [`references/content-brief-by-type.md`](references/content-brief-by-type.md).

| Rung | Job | Type | Draw when |
| --- | --- | --- | --- |
| 1. Overview | live entry, major components, externals, final result, composition gap | Architecture | always |
| 2. Layers | layers and dependency direction | Layer stack | the feature crosses layers |
| 3. Structure | central types and how they relate | ER (entities, cardinality), Database schema (real tables), UML class (protocols, inheritance, operations) | three or more central types |
| 4. Spine | real values in, the functions that transform them, values out; loops, fan-in, decisions, I/O, state changes | Sequence (call order is the point) or Data flow (custody and shape is the point) | always |
| 5. Lifecycle | states, guarded transitions, terminal outcomes | State machine | the feature owns a state field or status enum |
| 6. Zooms | one dense stage of the spine expanded | Flowchart, Sequence, or Data flow | a stage hides a decision tree or a loop |

Add any other diagram-design type when it proves a fact prose cannot: Dependency graph for fan-in, Swimlane for handoffs between processes, Deployment when the feature spans hosts.

The Draw-when column qualifies a rung; your judgment admits it. For each qualifying rung, ask what the reader would misunderstand without it: a real answer admits the figure, no answer skips it. Effort never decides in either direction. Four figures is the usual size; a Layers or Structure figure that only restates the Overview and the prose is skipped.

Per-figure content rules: [`references/authoring-invariants.md`](references/authoring-invariants.md) and [`references/content-brief-by-type.md`](references/content-brief-by-type.md).

Done when the figure list is appended to `brief.md`: for each, the file name, the type, the nodes, and the one fact it proves (diff variants included).

## 5. Draw and export

Figures are Python scripts over [`scripts/draw.py`](scripts/draw.py), a drawing kit whose primitives already satisfy diagram-design's default profile (palette, fonts, 4px grid, masked labels, orthogonal connectors, paint order). Check the project's `.diagram-design` marker: absent or `profile: default` uses the kit as is; any other profile means the kit's palette does not apply, so load diagram-design's `references/profiles.md` and pass the resolved tokens to the worker to override `draw.py`'s constants.

Spawn one drawing worker for all figures (two when there are four or more, split by rung; each figure script is ~80 lines of output, so the split halves wall-clock). Its inputs are exactly: `brief.md`, `scripts/draw.py` (the docstring is the API), `scripts/example-figure.py` (the shape of a figure script), and [`references/authoring-invariants.md`](references/authoring-invariants.md). It reads nothing from diagram-design. For each figure it writes `diagrams/<rung>-<name>.py` calling `write()`, which emits the `.html` and the standalone `.svg`, then runs `scripts/check-figures.sh diagrams/` once and fixes every failure it reports. It reports per figure: checks passed, node and arrow counts, and what it cut from the brief. It does not view the PNGs. Tell it: the check script finishes in about two seconds, so a Bash call that times out means the shell hung, not that a check failed; re-run the same command once in the background rather than decomposing the checks. Drawing figures yourself is the fallback for a single-figure walkthrough.

`check-figures.sh` also rasterized every figure into `diagrams/png/`. View each PNG once, the checks cannot see text collisions or a label crowding an edge, and fix what you find in the figure script, re-run the check, and stop.

Done when every listed figure has a `.py`, an `.html`, and an `.svg`, the check exits 0, and every PNG was viewed.

## 6. Write the walkthrough

Write `ai-docs/explanations/<feature-slug>/walkthrough.md` in the `writing-clearly-and-concisely` register, sized by relevant files: 1–3 files, 150–300 words; 4–10, 300–600; 11+, 500–900. The figures carry the structure; the prose carries anchors and the facts no figure can.

Open the document with the whole path in one sentence. Sections in order: **Context** (what starts the feature, what it produces, the scope, three lines at most; in diff mode, the range and two lines on what changed), one section per rung drawn in ladder order, **What changed** (diff mode: each diff figure with its text list), **Result** (the most important fact, one short paragraph).

Each rung section: one bold sentence stating the figure's fact, the figure embedded as `![caption](diagrams/<file>.svg)`, then at most five anchored facts. Rung 3 lists three to seven central types with anchors, grouped by layer for a large feature. Rung 4 is a numbered list of hops: per hop, the function, the value in, the value out. Anchor every structural claim with `file:line`; keep identifiers exact. When the user asks about private functions or reuse, list non-test call sites apart from test call sites.

Done when architecture and spine form one path, each call flow appears once, and every figure in `diagrams/` appears once.

## 7. Open for annotation

Run `python3 scripts/check-anchors.py <repo-root> walkthrough.md --quiet` (exit 0 or fix), then build `walkthrough.html` with `python3 scripts/build-html.py walkthrough.md`, which inlines every SVG per [`references/annotation-build.md`](references/annotation-build.md). Run `plannotator annotate walkthrough.html` in the background without a timeout; it blocks until the reader submits. Plannotator renders SVG only on its raw-HTML surface, so the `.md` stays the repo artifact and the `.html` is the one the reader opens. The reader annotates figures and prose and asks questions through Ask AI. Address the returned annotations in the same conversation, rebuilding the `.html` after any change; the `plannotator` skill holds the stdout contract.

In chat, give the result paragraph and the walkthrough path.

Done when the annotations are addressed or the reader closed the session.
