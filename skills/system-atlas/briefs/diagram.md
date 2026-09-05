# Diagram-building brief (read fully before starting)

You build a set of editorial diagrams for ONE section of the "<PRODUCT> System Atlas". You draw from the topic records assigned to you (and may open the source code under <REPO> to verify facts or fill gaps — do that when a topic fact is vague; never invent).

## Tooling — Diagram Design skill (mandatory)

Skill dir: <DIAGRAM_SKILL_DIR>
1. Follow Diagram Design's `SKILL.md`: choose a type, load its reference, and copy the closest HTML template.
2. Author the diagram directly in HTML with inline SVG. HTML is the editable visual source; the JSON sidecar below contains only semantic metadata, never coordinates or rendering instructions.
3. Run `python3 <DIAGRAM_SKILL_DIR>/scripts/self_check.py <name>.html`. Capture and inspect the rendered diagram at its intended viewing size. Fix clipped labels, overlaps, unreadable text and misleading connectors in the HTML, then check and inspect again. Do not deliver a diagram you did not inspect.

Read the caption shape in `briefs/caption.md` once and write final captions during construction; no separate caption worker is required.

## Output location and manifest

Write everything into your assigned directory `<OUTDIR>` (create it). Files: `NN-slug.json`, `NN-slug.html`, `NN-slug.png` with NN = 01, 02, ... in reading order.
Write early and refresh after every few diagrams: `<OUTDIR>/manifest.json`:
```json
{"section": "<section-id>", "title": "<Section title>", "order": <int given to you>,
 "intro": "2-4 sentences: what this section covers and how to read it (top-down).",
 "diagrams": [
   {"file": "01-slug.html", "json": "01-slug.json", "title": "...", "type": "sequence", "level": 1,
    "id": "stable-figure-id", "repo": "<repo-id>",
    "question": "One reader question this figure answers.",
    "caption": "Supportive text only: 2-6 sentences that explain how to read the diagram and the one or two facts a reader cannot see in it. Reference concrete file paths (path:line) where a reader should go next. Paragraphs separated by a blank line."}
 ]}
```
`level`: 1 = section overview (first 1-2 diagrams), 2 = one subsystem/flow, 3 = a detail inside a level-2 diagram. Order the list top-down: level 1 first, then each level-2 diagram followed immediately by its level-3 zooms.

## Consider every type

The orchestrator supplies the full type catalogue from Diagram Design's `SKILL.md` visual-type guide. Walk through it once before drawing. For every type, ask: does this section contain a structure, flow, state, hierarchy, time axis, comparison or quantity that this type explains better than any other? Use each type that answers yes; skip each that answers no. A substantial section often supports six or more types. Quantitative types require supported quantities in the topic records.

Record one compact `typeDecisions` entry per catalogue row in the manifest: type, chosen subject/figure, or why it does not apply. Record `coverage` mapping applicable subjects to figures, and a `depthCheck` identifying any remaining generic boxes. Target 12–20 figures for a substantial section. Fewer requires a gap check and `quotaReason`; more is allowed. A sequence diagram does not discharge a distinct state/model/ownership question. Each important opaque stage earns a zoom into real decisions, values, and failures.

## Quality rules

- Each figure answers one reader question, written as the first sentence of its caption. Two figures with the same question merge into the clearer one.
- Diagrams explain; text supports. Put the facts INTO the diagram (labels, sublabels with file/function names, table columns, edge labels with formats/keys), keep captions short.
- Respect each type's node/edge budget: when a subject exceeds it, split into an overview diagram + zoom diagrams (this is desired — we want depth). Never shrink fonts to fit.
- Use real names from the code: module names, function names, class names, S3 key patterns, table names, enum values, CLI flags. Put technical strings in secondary labels.
- Wide viewBoxes are fine (up to ~1600 wide); tall is fine too. Keep text ≥ 12px equivalent.
- Keep a semantic JSON sidecar with `nodes`, `edges`, `zones`, and `primitives` arrays as needed. Each entry has a stable unique `id`, `label`, optional `sublabel`, `repo`, and `code` source ranges; edges also have `from` and `to`. Link topics to figure IDs and sidecar paths. No geometry belongs in this inventory.
- On the corresponding SVG group or element, set `data-element-id` to the same ID. For existing source, set `data-repo` and `data-code="path:start-end"` (comma-separated for multiple ranges). Mirror these bindings in the sidecar's `code` array; ranges must match the topic's pinned sources. This enables source navigation and validation without a renderer.
- Aim for the number of diagrams requested in your task. More small clear diagrams beat one crowded one.
- Do not skip the PNG inspection step. Do not write any summary prose outside the manifest captions.

When done, reply with only: the OUTDIR, the number of diagrams built, and any facts you could not verify.
