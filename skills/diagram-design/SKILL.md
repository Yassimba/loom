---
name: diagram-design
description: Create or redraw polished architecture, process, data, UML, statistical, roadmap, and other diagrams as self-contained HTML/SVG/PNG. Use when a visual explains structure, flow, state, hierarchy, time, comparison, or causality better than prose.
---

# Diagram Design

Create editorial diagrams as self-contained HTML with inline SVG. Load only the references selected below; do not preload the reference directory.

## 0. Profile gate

Resolve a project-root `.diagram-design` marker per [`references/profiles.md`](references/profiles.md) when one exists. Otherwise inspect [`references/style-guide.md`](references/style-guide.md): if it still has the shipped defaults (`paper #f5f5f5`, `ink #2d3142`, `accent #eb6c36`), ask once whether to use the default or onboard a brand from a URL, installed skill, local design-system folder, pasted tokens, or saved profile. For onboarding, load [`references/onboarding.md`](references/onboarding.md). A customized style or explicit default choice skips this gate later.

## 1. Philosophy

Target density is 4/10. Merge ideas that always travel together; remove nodes, relationships, and labels already implied by layout. Accent is editorial: one or two focal elements, not a status system.

## 2. When to use

Draw only when a visual teaches more than a paragraph, table, or short Unicode sketch. Prefer prose for one idea, a table for aligned attributes, and two columns for simple before/after comparisons.

## 3. Select

1. Choose the obvious visual type. If ambiguous, load [`references/type-index.md`](references/type-index.md).
2. If behavior, enforcement, state, or risk carries the meaning, also load [`references/semantic-patterns.md`](references/semantic-patterns.md) and choose one primary pattern.
3. Load exactly one matching `references/type-<name>.md`. Its grammar and budget override generic defaults.
4. Use `doc-inline`, balanced detail, and mixed audience unless requested otherwise. For other output dials, load [`references/output-spec.md`](references/output-spec.md).
5. Before drawing, state type, semantic pattern if any, size, and anything omitted to meet the budget. Skip the pause when the request already fixes them.

## 4. Anti-patterns

Use borders, hierarchy, and whitespace. Avoid shadows, glow, 3-D effects, giant rounding, rainbow palettes, generic equal cards, blanket monospace, identical boxes for every role, and legends floating over the figure.

## 5. Design system

The active profile is the source of truth. Default roles are `paper`, `paper-2`, `ink`, `muted`, `soft`, `rule`, `rule-solid`, `accent`, `accent-tint`, and `link`. Use `link` for HTTP/API or external paths. Human names use Geist; technical strings use Geist Mono; page titles use Instrument Serif.

| Role | Treatment |
| --- | --- |
| focal | accent tint + accent stroke |
| backend/step | paper or white + ink stroke |
| store/state | faint ink fill + muted stroke |
| external | faint ink fill + translucent ink stroke |
| input/user | muted tint + soft stroke |
| optional/async | faint fill + dashed translucent stroke |
| security/boundary | accent tint + dashed accent stroke |

## 6. Connector contract

Type-specific primitives such as Sankey bands, fishbone bones, and loop arcs follow their type reference. Ordinary connections obey all six rules:

1. Off-axis routes use rounded orthogonal elbows (`r=8`; `r=6` only when tight). Straight lines require a shared x or y coordinate.
2. Labels use an opaque paper mask and sit 6–10px clear of the stroke; vertical labels sit beside it.
3. Paths never overlap. Offset parallel routes by at least 12px; unavoidable crossings use a bridge/hop.
4. Multiple paths on one node edge use distinct attach points at least 12px apart.
5. Route around non-endpoint nodes. An unavoidable transit behind one is dashed and keeps its label at the visible end.
6. A label mask must not overlap a node painted after it.

Draw in this order: background → zones → connectors and labels → nodes → legend. A standard node is an opaque paper mask, styled `rx=6` box, rectangular `rx=2` type tag, Geist name, and optional Geist Mono sublabel. Define default, accent, and link arrow markers when the diagram uses directed connectors. Load `references/type-architecture.md` when a selected type needs elbow, port-selection, or bridge formulas and does not define them itself.

## 7. Layout and budget

Use a 4px grid for coordinates, dimensions, padding, gaps, and font sizes. Allowed radii are 4, 6, and 8px; stroke widths and data-derived coordinates are exempt. Default budget: 9 nodes, 12 connectors, 2 accent elements. Over budget means overview + detail, not smaller text. Type references may set tighter or explicit larger budgets.

Put legends in a separated bottom strip. Every meaningful SVG has a file-prefixed accessible name, with `title` first:

```html
<svg role="img" aria-labelledby="slug-title slug-desc">
  <title id="slug-title">Short subject</title>
  <desc id="slug-desc">One sentence describing the useful meaning.</desc>
```

Decorative SVGs use `aria-hidden="true"`.

## 8. Templates

After selecting any type, prefer the compact renderer: load [`references/render-spec.md`](references/render-spec.md), author JSON with explicit geometry, then run `scripts/render.py`. Its universal primitives support all types; rendering convenience must never influence type selection. Keep JSON as the editable source and use handwritten SVG only when a required effect cannot be expressed.

Otherwise copy the nearest asset instead of writing page chrome from memory:

- minimal light: `assets/template.html`
- minimal dark: `assets/template-dark.html`
- full editorial: `assets/template-full.html`
- motion: `assets/template-motion.html`
- terminal: `assets/template-terminal.html`

Replace title, slug, description, and SVG body with the selected type's content.

## 9. Validate

Run every output through the installed checks:

```bash
python3 <skill-dir>/scripts/check.py path/to/diagram.html
```

Then inspect the rendering once. Confirm type fit, readable hierarchy, budget, connector traceability, unclipped labels, and restrained accent. Fix findings before delivery. Animated work must also preserve a complete static, print, no-JS, and reduced-motion frame.

## 10. Optional variants

Load only when selected: motion → [`animation.md`](references/animation.md); terminal → [`primitive-terminal.md`](references/primitive-terminal.md); icons → [`primitive-icons.md`](references/primitive-icons.md); callouts → [`primitive-annotation.md`](references/primitive-annotation.md); hand-drawn styling → [`primitive-sketchy.md`](references/primitive-sketchy.md).

## 11. Imports

For draw.io, load [`references/import-drawio.md`](references/import-drawio.md) and run `<skill-dir>/scripts/drawio_extract.py`. For Mermaid, load [`references/import-mermaid.md`](references/import-mermaid.md) and run `<skill-dir>/scripts/mermaid_extract.py`. Extract structure, then redraw; discard renderer coordinates and styling. Set format, size, detail, and audience first, and report what was merged, collapsed, or dropped.

## 12. Output

Default output is one self-contained `.html` file with embedded CSS and inline SVG; Google Fonts is the only permitted external asset. JavaScript is allowed only for requested motion and must use the canonical controller from the motion template.

Export only when requested. Load [`references/export.md`](references/export.md); HTML remains the source of truth, while SVG/PNG exports contain the diagram alone.