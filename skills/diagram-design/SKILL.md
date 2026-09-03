---
name: diagram-design
description: Create or redraw polished architecture, process, data, UML, statistical, roadmap, and other diagrams as self-contained HTML/SVG/PNG. Use when a visual explains structure, flow, state, hierarchy, time, comparison, or causality better than prose.
---

# Diagram Design

Create editorial diagrams as self-contained HTML with inline SVG. Target density is 4/10: merge ideas that travel together, remove labels implied by layout, and reserve accent for one or two focal elements.

## 1. Select

1. Draw when a visual teaches more than a paragraph, table, or short Unicode sketch.
2. Choose the obvious visual type. When the user names it, do not load the type index; when the fit is ambiguous, load [`references/type-index.md`](references/type-index.md).
3. For behavior, enforcement, state, or risk, also load [`references/semantic-patterns.md`](references/semantic-patterns.md) and choose one primary pattern.
4. Load exactly one selected `references/type-<name>.md`; its grammar and budget win.
5. Use `doc-inline`, balanced detail, and mixed audience. Load [`references/output-spec.md`](references/output-spec.md) only when the user requests different output dials.
6. State the selected type, semantic pattern, size, and omitted detail before drawing when the request leaves those choices open.

## 2. Compose

Use borders, hierarchy, and whitespace. Build an editorial composition with one obvious reading order. Use the active profile's semantic roles rather than literal colors.

Avoid shadows, glow, 3-D effects, giant rounding, rainbow palettes, generic equal cards, blanket monospace, identical boxes for every role, and legends over the figure.

For ordinary connected nodes, load [`references/connected-layout.md`](references/connected-layout.md). Specialized connectors and geometry follow the selected type reference.

Every meaningful SVG has a file-prefixed accessible name with `title` first:

```html
<svg role="img" aria-labelledby="slug-title slug-desc">
  <title id="slug-title">Short subject</title>
  <desc id="slug-desc">One sentence describing the useful meaning.</desc>
```

Decorative SVGs use `aria-hidden="true"`.

## 3. Build

Prefer the compact renderer. Load [`references/render-spec.md`](references/render-spec.md), author JSON with explicit geometry, then run one build gate:

```bash
python3 <skill-dir>/scripts/build.py diagram.json \
  --project-root <project-root> \
  --output diagram.html \
  --inspect diagram.png
```

The documented command is the complete renderer interface; execute it without reading its implementation or example HTML. Use renderer-native recipes in the selected type reference when a shape or route is unclear; handwritten examples are for unsupported effects. The build resolves the profile, renders, runs every installed check, and captures the inspection PNG. Inspect the PNG and fix visible defects with a targeted JSON change, then rebuild and inspect again. Once the build and inspected rendering are clean, deliver; additional assertions, greps, direct checks, repository-status checks, and speculative redesigns are redundant. Keep JSON as the editable source and HTML as the source of truth.

Only when a required effect cannot be expressed by the renderer, switch to handwritten HTML and resolve the active profile with:

```bash
python3 <skill-dir>/scripts/diagram_profile.py --project-root <project-root>
```

Then copy the nearest asset instead of recreating page chrome:

- minimal light: `assets/template.html`
- minimal dark: `assets/template-dark.html`
- full editorial: `assets/template-full.html`
- motion: `assets/template-motion.html`
- terminal: `assets/template-terminal.html`

Run handwritten HTML through `scripts/check.py`, capture one rendered inspection, and fix findings before delivery.

Profile management (`save`, `load`/`switch`, `list`, `show`, `update`, `reset`, `delete`) uses [`references/profiles.md`](references/profiles.md). Generation uses the resolver and does not load that management reference.

## 4. Conditional branches

Load only the selected branch:

- motion → [`references/animation.md`](references/animation.md)
- terminal → [`references/primitive-terminal.md`](references/primitive-terminal.md)
- icons → [`references/primitive-icons.md`](references/primitive-icons.md)
- callouts → [`references/primitive-annotation.md`](references/primitive-annotation.md)
- hand-drawn styling → [`references/primitive-sketchy.md`](references/primitive-sketchy.md)
- draw.io import → [`references/import-drawio.md`](references/import-drawio.md), then `scripts/drawio_extract.py`
- Mermaid import → [`references/import-mermaid.md`](references/import-mermaid.md), then `scripts/mermaid_extract.py`

Imports preserve structure and redraw it in the selected grammar; renderer coordinates and styling are discarded.

## 5. Deliver

Default output is one self-contained `.html` file with embedded CSS and inline SVG. Google Fonts is the only permitted external asset. JavaScript is reserved for requested motion and uses the canonical motion controller.

Export only when requested. Load [`references/export.md`](references/export.md); SVG and PNG exports contain the diagram alone while HTML remains the source of truth.

Done when the selected type fits, hierarchy reads clearly, labels remain unclipped, connectors are traceable, the budget and accent are restrained, the build gate passes, and the inspected rendering has no visible defect.
