---
name: changeset-walkthrough
description: "Guided walkthrough of a git change, chaptered, with diagram-design figures showing what moved and what stayed. Every box opens its code in Plannotator, annotatable. Use to review a branch, PR, or range visually rather than file by file, or when a diff is too large to understand in path order."
---

# Changeset Walkthrough

Explain a change the way its author reasoned about it: one document of ordered chapters, each carrying figures that show the system's shape with the change colored onto it. The reader clicks any box, or an anchored phrase in the prose, and that code opens in a panel beside the document, scrolled to the line, ready to annotate. The picture is the context the code sits in.

This is `explain-code-flow`'s evidence discipline pointed at a diff, delivered through Plannotator's Guided Review so the code is live rather than pasted. Where that skill climbs a fixed ladder of figures, this one enumerates the whole diagram catalogue and records a verdict for each — a changeset's best figure is rarely the obvious one.

**Not a bug hunt.** You are chaptering and drawing a change, not auditing it. Mention a bug you trip over in one clause and move on; most chapters mention none.

## 1. Pin the range

Resolve `from` and `to` to real commits (`to` defaults to the working tree). A branch means its merge-base with trunk; a PR means its base and head. Capture the patch once and work from that copy only — the reader can move their checkout while you write:

```bash
git diff <from>...<to> > /tmp/walkthrough.patch
git diff --name-only <from>...<to>
```

That file list is authoritative for the rest of the run. Every path you write must appear on it, spelled identically.

Done when `from`, `to`, the patch file, and the changed-file list are fixed.

## 2. Map the evidence

When the repo has a `.codegraph/` index, query it first, once, scoped to the areas the patch touches. Otherwise give one Explore worker `explain-code-flow`'s [`references/repository-evidence.md`](../explain-code-flow/references/repository-evidence.md) — its inspection order and Deliverables are the worker's contract.

Copy every anchor from a `grep -n` or `sed -n` result line; never type one from memory. Write the map to `ai-docs/walkthroughs/<slug>/brief.md` with each node's change class beside its anchor. That file is the drawing workers' whole input, so keep it to what a figure needs: every kilobyte costs each writer about four seconds, and a changeset walkthrough spawns one worker per figure.

Then compute the change per [`references/diagram-diff.md`](../explain-code-flow/references/diagram-diff.md) §"Compute the change": classify every node and edge as **added**, **removed**, **changed**, or **unchanged**. The unchanged set is not filler — showing what held still is half of what makes a diff legible.

When the repo has a `.codegraph/` index, query it first; `calldiff diff <from> <to> --entry <entry> --locs` gives the moved-callee list directly.

Done when every changed file is accounted for, every drawn edge has an anchor, and every node carries a change class.

## 3. Verify

Run `python3 ../explain-code-flow/scripts/check-anchors.py <repo-root> brief.md`: it resolves every `file:line` and prints the source line beside the claim, so drift is visible without opening files. Scan the list once and fix any anchor whose printed line does not carry the claimed symbol.

Anchors at `from` need the other revision — `git show <from>:<path>` — since `check-anchors.py` reads the working tree. Verify each **added** and **removed** item that way. Draw an edge only once anchored; otherwise label it an assumption.

Done when the check exits 0 and every printed line supports its claim.

## 4. Chapter the change

Order chapters by importance, not by path or diff size: the implementation heart first, then its consequences in decreasing signal, then one trailing chapter grouping glue, wiring, and config under an honest title.

Chunk by logical unit. Three files changed for one reason are one chapter; one file changed for two reasons is two chapters; unrelated work never shares a chapter.

**Coverage is a hard constraint.** Every changed file appears in exactly one chapter's `diffs`, or in `unplacedFiles`. Never both, never twice, never omitted. Plannotator drops refs that break this and fails the import if nothing survives.

Two to six chapters. Never more than ten.

Done when every path from step 1 is placed exactly once.

## 5. Choose the figures

Figure selection is an **enumeration, not a shortlist**. A shortlist finds the obvious figure and misses the one that would have prevented a misreading — the Layer stack that shows why a package boundary holds, the Venn that shows what actually crosses a sanitizer. So walk the whole catalogue and write a verdict for every type.

### Enumerate

Open diagram-design's type-selection table (its SKILL.md) and its semantic-pattern trigger table. **Count the rows.** Your record gets one line per type in that table — if the table lists 39 types, the record has 39 lines. Never omit a type because it is obviously wrong; "obviously wrong" is the verdict, and writing it is what proves you looked.

For each type, ask exactly one question:

> Would a reader misunderstand something about this change without this figure?

- **DRAW** — name the misreading it prevents, the chapter it belongs to, and its nodes. A verdict with no named misreading is not a DRAW; delete it.
- **SKIP** — one clause. "No quantitative data." "One actor, no handoffs." "No status field." Enough that a reader can tell refusal from oversight.

Effort decides nothing in either direction, and neither does a target count. Six figures for a fifteen-file change is as legitimate as one, if six misreadings are named.

### Write the record

Keep it beside the walkthrough as `figure-selection.md`:

```markdown
| Type | Verdict | Reason |
| --- | --- | --- |
| Data flow | DRAW ch2 | Four new steps feed four that already existed; the boundary is the point. |
| Sequence | DRAW ch4 | Patch-before-envelope is ordering, not style — and a crash between them must stay safe. |
| Venn | DRAW ch2 | What crosses the sanitizer: script/onclick out, data-code/style in. |
| Sankey | SKIP | Nothing quantitative splits or merges. |
| Fishbone | SKIP | Root-cause analysis of a defect; there is no defect. |
```

Two chapters with no figure and stated reasons is a result. Two chapters with no figure and no reasoning is an omission, and only the record tells them apart. It is also what a reader argues with — "why no state machine?" has an answer on the page.

### Guard both directions

The enumeration exists to stop under-drawing. It must not start over-drawing:

- A figure that restates the chapter's file list is worse than no figure.
- Two figures proving one fact is worse than one. When two types both fit, pick the one that carries it smaller and SKIP the other *naming the winner* — "the ER already carries it".
- A chapter earns figures on its own; do not spread them evenly to look thorough.

Draw each figure as it is **after** the change, with the change coloured onto it — one figure, not a before-and-after pair. Removed nodes are drawn back in their former position so the reader sees what left.

Done when the record has a line for every type in diagram-design's table, every DRAW names its misreading and chapter, and every SKIP names its reason.

## 6. Draw

Figures are Python scripts over `explain-code-flow`'s [`scripts/draw.py`](../explain-code-flow/scripts/draw.py), a kit whose primitives already satisfy diagram-design's default profile — palette, fonts, 4px grid, masked labels, orthogonal connectors, paint order. **The workers read nothing from diagram-design.** The kit also ships this skill's palette natively:

```python
ADDED, REMOVED, CHANGED = "#2f7d4f", "#b3382c", "#b7791f"
```

Check the project's `.diagram-design` marker: absent or `profile: default` uses the kit as is; any other profile means its palette does not apply, so load diagram-design's `references/profiles.md` and pass the resolved tokens to override the kit's constants.

**Spawn one worker per figure, all in the same message so they run in parallel.** A worker's time is dominated by planning one figure's coordinates, about 100 seconds regardless of effort, so six sequential figures cost thirteen minutes and six parallel ones cost two. Each worker's inputs are exactly: `brief.md`, `scripts/draw.py` (its docstring is the API), `scripts/example-figure.py`, [`references/authoring-invariants.md`](../explain-code-flow/references/authoring-invariants.md), and the two rules below.

**Colour the change.** Use the kit's `ADDED` / `REMOVED` / `CHANGED` constants per [`references/diagram-diff.md`](../explain-code-flow/references/diagram-diff.md). Colour never carries a class alone — repeat it as a `+` / `−` / `~` tag so the figure survives greyscale. This convention replaces the kit's focal-accent rule: `ACCENT` appears nowhere in a diff figure. A legend is mandatory, one row per class present, plus a searchable text list of the changes with their anchors.

**Draw the unchanged at equal weight.** Unchanged nodes are the point — they say the existing machinery is the destination. Same size, same prominence, just uncoloured and untagged.

**Bind every box that stands for a changed file.** Any node whose brief line names a path on the step-1 file list carries `data-code` with that path, exactly as the brief spells it, plus the anchor's line range so the click lands on the code rather than the file header; comma-separated for several, first one primary. A node standing for a concept, or for a file outside the patch, stays unbound — an unbound box is context, a bound one is a door, and a door onto a file the guide cannot open is worse than no door.

```html
<g data-code="packages/server/review.ts:40-66">
```

Workers write `ai-docs/walkthroughs/<slug>/diagrams/<chapter>-<name>.py` calling `write()`, which emits the `.html` and the standalone `.svg`. When they are all back, run `../explain-code-flow/scripts/check-figures.sh diagrams/` once: it runs both mechanical checks and rasterizes every figure into `diagrams/png/`. Fix what it reports.

Then **view every PNG once**. The checks cannot see a text collision or a label crowding an edge, and a worker's own density estimate is not a substitute for looking. Fix in the figure script, re-run the check, stop.

Finally confirm the bindings survived export: `grep -c '<g data-code=' diagrams/*.svg`. A figure whose count is zero is decoration.

Done when every figure has a `.py`, an `.html`, and an `.svg`, the check exits 0, every PNG was viewed, and every figure's binding count is non-zero.

## 7. Write the guide

Write `ai-docs/walkthroughs/<slug>/guide.json`:

```json
{
  "title": "...",
  "intent": "1-2 sentences: why this changeset exists",
  "sections": [
    {
      "title": "Concept-level, never a filename paraphrase",
      "overview": "2-6 sentences: what changed, why, what it implies.",
      "diagrams": ["<svg viewBox=...>...</svg>"],
      "diffs": [{ "file": "exact/path.ts", "summary": "1-2 sentences from the hunks alone." }]
    }
  ],
  "unplacedFiles": []
}
```

`diagrams` holds the chapter's exported SVG **markup inline**, in reading order — read each `.svg` and drop the `<?xml …?>` declaration. Verify each string still contains its `data-code` attributes after embedding; a figure that lost them is a picture, not a walkthrough. Scripts, event handlers, and `foreignObject` are stripped when rendered; `data-code`, `<style>`, and inline styles survive.

Anchor the prose too. A phrase that makes a claim about one place in the code links to it with `[phrase](code:path:from-to)`, the path spelled as in the file list and the range copied from the brief; the reader clicks it and the code opens beside the document like a figure box. One or two per chapter, on the claims a reader would want to check, never on every file name.

Prose register: the `i-have-adhd` shape, because the reader holds nothing between chapters. The first sentence of an overview names the thing to look at, in the figure or the diff. Then at most three sentences, one idea each, 25 words or fewer. A sequence of steps is a numbered list, one bounded action per item, never more than five. Plain words — file, function, the server. Code names in backticks, at most two per sentence, and the sentence still reads as English with them covered. No verdicts: not *elegant*, *robust*, *seamless*, *simply*. No em-dashes, no preamble, no recap. The figures and the diff carry the shape; your words carry the why.

Reading loop the guide is built for: read the overview, click the figure to enlarge it, click a box to land in its code beside the document, `⇧Z` to zoom the code, comment, `Esc`, scroll to the next figure. Write each chapter so that loop works: the overview points at one box to click first.

Done when every changed file is placed once and every chapter's `diagrams` parse as SVG.

## 8. Open it

```bash
plannotator guide import --guide ai-docs/walkthroughs/<slug>/guide.json --patch /tmp/walkthrough.patch
plannotator review
```

`import` puts the guide on the local shelf and prints its id; it validates strictly, so an unknown or twice-placed path is an error naming the file and listing the patch's real paths — fix `guide.json` and re-import. The guide then opens from **Previous guides**, live, as one document with a contents rail: click a figure element or a prose anchor, its code opens in the panel beside the document at that line, annotate it there.

Run `plannotator review` without a timeout; it blocks until the reader submits. Address returned annotations in the same conversation and re-import after any change. The `plannotator` skill holds the stdout contract.

In chat, give the guide id and the walkthrough path.

Done when the annotations are addressed or the reader closed the session.
