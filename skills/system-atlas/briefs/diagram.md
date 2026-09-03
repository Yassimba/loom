# Diagram-building brief (read fully before starting)

You build a set of editorial diagrams for ONE section of the "<PRODUCT> System Atlas". You draw from the NOTES file assigned to you (and may open the source code under <REPO> to verify facts or fill gaps — do that when a notes item is vague; never invent).

## Tooling — Diagram Design skill (mandatory)

Skill dir: <DIAGRAM_SKILL_DIR>
1. Read `references/render-spec.md` (JSON renderer contract) and `references/connected-layout.md`.
2. For EACH diagram type you use, read exactly its `references/type-<name>.md` once (e.g. type-sequence.md, type-er.md, type-swimlane.md, type-state.md, type-flowchart.md, type-data-flow.md, type-process.md, type-uml-class.md, type-dependency.md, type-tree.md, type-nested.md, type-layers.md, type-deployment.md, type-architecture.md, type-db-schema.md, type-timeline.md, type-gantt.md, type-sankey.md, type-treemap.md, type-medallion.md, type-dp-integration.md, type-high-level.md, type-loop.md, type-org-chart.md, type-quadrant.md, type-radar.md, type-bar.md, type-line.md, type-slopegraph.md, type-fishbone.md, type-kanban.md, type-journey.md, type-story-map.md, type-venn.md, type-pyramid.md, type-wardley.md). The type reference's grammar and budget win. Renderer-native recipes in the type reference show how to express that type with `primitives`.
3. Author one JSON per diagram with explicit geometry (4px grid), then build:
   ```
   python3 <DIAGRAM_SKILL_DIR>/scripts/build.py <name>.json --project-root <PROJECT_ROOT> --output <name>.html --inspect <name>.png
   ```
   Then LOOK at the PNG (Read tool on the png). Fix overlaps, clipped labels, crossing/unclear connectors, text running outside boxes, by editing the JSON and rebuilding. Repeat until clean. Do not deliver a diagram whose PNG you did not inspect.
4. Every built diagram must pass the build gate (it prints "Diagram Design build passed").

## Output location and manifest

Write everything into your assigned directory `<OUTDIR>` (create it). Files: `NN-slug.json`, `NN-slug.html`, `NN-slug.png` with NN = 01, 02, ... in reading order.
Finally write `<OUTDIR>/manifest.json`:
```json
{"section": "<section-id>", "title": "<Section title>", "order": <int given to you>,
 "intro": "2-4 sentences: what this section covers and how to read it (top-down).",
 "diagrams": [
   {"file": "01-slug.html", "title": "...", "type": "sequence", "level": 1,
    "caption": "Supportive text only: 2-6 sentences that explain how to read the diagram and the one or two facts a reader cannot see in it. Reference concrete file paths (path:line) where a reader should go next. Paragraphs separated by a blank line."}
 ]}
```
`level`: 1 = section overview (first 1-2 diagrams), 2 = one subsystem/flow, 3 = a detail inside a level-2 diagram. Order the list top-down: level 1 first, then each level-2 diagram followed immediately by its level-3 zooms.

## Consider every type

The orchestrator pastes the full type catalogue from `references/type-index.md` into your task. Walk through it once before you start. For every type, ask: does this section contain a structure, flow, state, hierarchy, time axis, comparison or quantity that this type would explain better than any other? Use each type that answers yes. Cover at least: architecture, data flow, sequence, swimlane, process or flowchart, state machine, ER or db-schema, UML class, dependency graph, tree or nested, deployment or layers, and one quantitative type (sankey, treemap, bar, timeline or gantt) where the notes give numbers.

## Quality rules

- Diagrams explain; text supports. Put the facts INTO the diagram (labels, sublabels with file/function names, table columns, edge labels with formats/keys), keep captions short.
- Respect each type's node/edge budget: when a subject exceeds it, split into an overview diagram + zoom diagrams (this is desired — we want depth). Never shrink fonts to fit.
- Use real names from the code: module names, function names, class names, S3 key patterns, table names, enum values, CLI flags. Use `sublabel` for the technical string.
- Wide viewBoxes are fine (up to ~1600 wide); tall is fine too. Keep text ≥ 12px equivalent.
- Roles: `focal` only for the 1-2 elements the diagram is about; `store` for S3/DB/files; `external` for other repos/systems; `input` for users/triggers; `optional` dashed for async/optional.
- Aim for the number of diagrams requested in your task. More small clear diagrams beat one crowded one.
- Do not skip the PNG inspection step. Do not write any summary prose outside the manifest captions.

When done, reply with only: the OUTDIR, the number of diagrams built, and any facts you could not verify.
