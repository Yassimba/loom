# Diagram Slides

Read this before composing the body of a content slide. A deck's default body is a drawing, not a bullet list — bullets are what a slide falls back to when its idea genuinely has no shape. Diagrams come from the `/diagram-design` skill and land in the deck as inline SVG.

## Choosing the form

Pick the form from what the content **is**, not from what you drew last time. Read the idea, name its shape, then pick:

| The idea is…                              | Draw it as                       |
| ----------------------------------------- | -------------------------------- |
| Parts of a system and what talks to what  | architecture, dependency graph   |
| One thing moving through stages           | flowchart, data flow, user journey |
| Who does what, in order, across actors    | sequence, swimlane               |
| A thing that can be in one of N states    | state machine                    |
| Strictly stacked levels, each on the one below | layer stack, medallion      |
| Two axes and where things land on them    | quadrant, scatter, Wardley map   |
| Overlap and what is shared                | Venn, nested                     |
| Narrowing from many to few                | funnel, pyramid, Sankey          |
| Events against dates                      | timeline, Gantt, roadmap         |
| Containment or hierarchy                  | tree, org chart, treemap, ER     |
| A cycle with no end                       | loop, flywheel                   |
| Causes behind one effect                  | fishbone                         |
| Amounts to compare, or a trend            | bar, line, radar                 |

## Varying the forms

One deck that draws every idea as boxes-and-arrows teaches less than a deck whose forms change with its content — the reader learns to read the shape itself. Keep a running list of the forms already used, and before drawing each new one, name that list and pick outside it.

Tag every figure with the form it uses — `<svg data-diagram="sequence" …>` — so the deck states its own variety and `scripts/check-deck.py` can hold these two bounds:

- No form appears on two consecutive diagram slides.
- A deck with six or more diagram slides carries at least five distinct forms.

If the content genuinely repeats a shape past those bounds — three sequences in a row because the deck really is three protocols — keep the honest form and change something else instead: the orientation, the density, the axis, or split the slide.

## Authoring

1. Invoke `/diagram-design` with the diagram's content **and the deck's design tokens** — the exact font families and hex palette of the chosen style — so the diagram is drawn in the deck's system.
2. Ask it to export SVG. Its export procedure writes a `.svg` beside the generated HTML. SVG only: a PNG blurs on a 1920×1080 stage scaled to a 4K display.
3. Open the `.svg` and paste its `<svg>` node into the slide markup. An inline node scales with the stage, inherits the deck's fonts, and exposes its groups to the deck's reveal animations — all three are lost through `<img src>`.

## Inlining

- Delete the exported `<defs><style>@import ...</style></defs>` font block. The deck already loads those fonts, and the duplicate import stalls first paint.
- Keep the `viewBox`, drop any fixed `width`/`height`, and size the node in CSS: `width: 100%; height: auto` inside the slide's grid cell.
- Keep the exported `<title>`, `<desc>`, and ID prefixes verbatim — the prefixes are what let two diagrams share one deck without one figure resolving to the other's gradients or accessible name.
- Animate by putting the deck's reveal classes on top-level `<g>` groups with staggered delays. The geometry stays as diagram-design authored it.

## Verification

Run `scripts/check-deck.py` and screenshot every diagram slide in the browser. A diagram is done when its text renders in the deck's fonts (not a fallback), nothing overflows the stage, and it reads at presentation distance — smallest label ≥ 18px at 1920×1080. Then check the deck as a whole against the two bounds above.
