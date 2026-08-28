---
name: archify-editorial
description: Render any archify spec (architecture, workflow, sequence, dataflow, lifecycle; typed JSON, validated routing) in the diagram-design editorial skin (paper/ink/accent tokens, Geist + Instrument Serif, rounded elbows, zones, legend strip). Use when the user wants archify's diagram types and validation but diagram-design's look, or asks to "render this archify spec editorially".
---

# archify-editorial

Two skills, one pipeline. archify owns **what** is drawn; diagram-design owns **how it looks**.

1. Author and validate the spec with the `archify` skill as usual:
   `node <skills>/archify/bin/archify.mjs validate architecture <spec.json> --quality showcase --json`
2. Render the editorial HTML:
   `python3 <skills>/archify-editorial/render.py <spec.json> <out.html> [--focal id,id] [--dark] [--eyebrow "Text"]`
3. Verify with diagram-design's own gates:
   `python3 <skills>/diagram-design/scripts/self_check.py <out.html>`
   `python3 <skills>/diagram-design/scripts/verify-geometry.py <out.html>`

The renderer validates the spec with archify, renders it once with archify's own renderer, and scrapes the geometry from that SVG (`data-node-id` nodes, `data-composition-points` edges, `data-composition-frame-kind` frames; untagged lines and rectangles are chrome such as lifelines, rails, and activation bars). Routes, frames, and label rectangles are therefore the ones archify already proved collision-free. A spec that fails archify validation is refused; fix the spec, never the HTML.

All five archify types work: `architecture`, `workflow`, `sequence`, `dataflow`, `lifecycle`. The `<type>` is read from the spec's `diagram_type`.

## Mapping

| archify                                            | diagram-design                                    |
| -------------------------------------------------- | ------------------------------------------------- |
| `frontend`, `backend`, `process`                   | backend (white / ink) — tags `UI`, `SVC`, `STEP`  |
| lifecycle `start`                                  | input (ink 6%) — tag `START`                      |
| lifecycle `active`, `decision`                     | backend — tags `STATE`, `GATE`                    |
| lifecycle `waiting`                                | external — tag `WAIT`                             |
| lifecycle `success`, workflow `end`                | store — tags `DONE`, `END`                        |
| lifecycle `failure`                                | security — tag `FAIL`                             |
| `database`, `messagebus`                           | store (ink 5%) — tags `DB`, `BUS`                 |
| `cloud`, `external`                                | external (ink 3% / ink 30%) — tags `CLOUD`, `EXT` |
| `security`                                         | security (accent 5%, dashed) — tag `SEC`          |
| `--focal id` (max 2)                               | accent-tint fill, accent stroke                   |
| connection `emphasis`                              | ink stroke 1.4                                    |
| connection `dashed`                                | muted, dashed 4,3                                 |
| connection `security`                              | accent, dashed 4,4                                |
| boundary `region`, lanes, stages, segments, groups | zone, ink wash                                    |
| boundary `security-group`, exception lanes         | zone, accent wash, dashed                         |
| lifelines, phase rails                             | muted dashed lines                                |
| sequence activations                               | store-filled bars                                 |
| `cards`                                            | summary cards below the diagram                   |

## Limits

- Workflow group labels keep archify's placement; a long group label can touch the first node in its group, as it does in archify itself. Shorten the label in the spec.
- No archify viewer: no pan/zoom, search, guided views, or export menu. Static HTML like every diagram-design output.
- diagram-design's 4px grid and 9-node budget are not enforced; archify's `showcase` (12 nodes) is the budget.
- Long sublabels can exceed the node width; shorten them in the spec.
