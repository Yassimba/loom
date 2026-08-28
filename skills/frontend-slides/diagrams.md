# Diagram Slides

Read this when a slide carries a diagram — architecture, flow, sequence, timeline, quadrant, funnel, Venn, org chart, Sankey, state machine. Diagrams come from the `/diagram-design` skill and land in the deck as inline SVG.

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

Screenshot every diagram slide in the browser. A diagram is done when its text renders in the deck's fonts (not a fallback), nothing overflows the stage, and it reads at presentation distance — smallest label ≥ 18px at 1920×1080.
