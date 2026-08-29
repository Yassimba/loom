# Compact renderer

The renderer supports every type in `type-index.md`. Type selection happens first; rendering must not influence it. It expands repetitive HTML/SVG while the agent keeps control of coordinates, data encoding, and route geometry.

```json
{
  "version": 1,
  "type": "architecture",
  "slug": "request-flow",
  "title": "Request flow",
  "description": "Request flow from browser through API to storage.",
  "viewBox": [0, 0, 1000, 600],
  "zones": [{"label": "PRIVATE", "x": 320, "y": 64, "width": 560, "height": 400}],
  "nodes": [
    {"id": "web", "label": "Browser", "tag": "USER", "x": 80, "y": 160, "width": 160, "height": 80, "role": "input"},
    {"id": "api", "label": "API", "sublabel": "https:443", "tag": "API", "x": 400, "y": 160, "width": 160, "height": 80, "role": "focal"}
  ],
  "edges": [
    {"from": "web:right", "to": "api:left", "tone": "link", "label": {"text": "HTTPS", "x": 320, "y": 140}}
  ],
  "legend": [{"role": "focal", "label": "Primary service"}]
}
```

Run:

```bash
python3 <skill-dir>/scripts/render.py diagram.json --output diagram.html
```

## Box-and-arrow conveniences

- Node roles: `focal`, `backend`, `step`, `store`, `external`, `input`, `optional`, `security`.
- Endpoints use `node-id:left|right|top|bottom`; `fromOffset` and `toOffset` fan attach points from `0` to `1`.
- Off-axis edges add `via: [[x,y], ...]`. Segments must be horizontal or vertical; corners are rounded.
- Edge tones: `default`, `accent`, `link`; `dashed: true` marks optional, async, or transit paths.
- Label `x` and `y` are the text baseline; the renderer adds the paper mask.

## Universal primitives

Use `primitives` for charts and specialized grammars. Items render in listed order. A diagram may use primitives alone or as a background beneath convenience zones, edges, and nodes.

| `kind` | Geometry |
| --- | --- |
| `rect` | `x`, `y`, `width`, `height`, optional `radius` |
| `circle` | `cx`, `cy`, `r` |
| `ellipse` | `cx`, `cy`, `rx`, `ry` |
| `line` | `x1`, `y1`, `x2`, `y2` |
| `polyline`, `polygon` | `points: [[x,y], ...]` |
| `path` | SVG `d`; use for arcs, ribbons, curves, and specialized marks |
| `text` | `x`, `y`, `text`; optional `font`, `size`, `weight`, `anchor`, `italic`, `letterSpacing`, `rotate` |

Shape styling accepts `fill`, `stroke`, `strokeWidth`, `opacity`, `dash`, `marker`, `linecap`, and `linejoin`. Paints may be semantic tokens, safe CSS colors, `none`, or compact tints such as `ink@5` and `accent@12`. Markers are `default`, `accent`, or `link`.

This vocabulary expresses every type without auto-layout: paths cover Sankey bands and loop arcs; lines and polygons cover fishbones, axes, radar, and Wardley maps; rectangles cover lanes, cards, tables, Gantt tasks, and treemaps; circles and ellipses cover scatter, Venn, and state marks.

## Contract

Required: `version`, `type`, `slug`, `title`, `description`. `nodes`, `edges`, `zones`, `primitives`, `legend`, `viewBox`, and semantic `tokens` overrides are optional. Coordinates use the 4px grid except data-derived marks. Keep the JSON as source; do not read generated HTML back into context. Run `scripts/check.py`, then inspect the rendering.