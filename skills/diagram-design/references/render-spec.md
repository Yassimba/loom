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

Build and inspect:

```bash
python3 <skill-dir>/scripts/build.py diagram.json \
  --project-root <project-root> \
  --output diagram.html \
  --inspect diagram.png
```

This documented interface is complete; run it without reading `build.py`, `render.py`, `check.py`, or example HTML. The build resolves the active profile, renders the HTML, runs `check.py`, and captures the PNG. Inspect the PNG; when it has a visible defect, make a targeted JSON change, rebuild, and inspect again. Deliver once the build and rendering are clean. Use `render.py` directly only when another workflow owns validation and inspection.

## Box-and-arrow conveniences

- Node roles: `focal`, `backend`, `step`, `store`, `external`, `input`, `optional`, `security`.
- Node shapes: `rectangle` (default) and `diamond` for decisions.
- Endpoints use `node-id:left|right|top|bottom`; `fromOffset` and `toOffset` fan attach points from `0` to `1`.
- Off-axis edges add `via: [[x,y], ...]`. Segments must be horizontal or vertical; corners are rounded. Repeated points are ignored, and diagonal errors name the edge plus two valid waypoint choices.
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
| `text` | `x`, `y`, `text`; optional `font` (`sans`, `mono`, or `serif`), numeric `size`, `weight`, `anchor`, `italic`, numeric `letterSpacing`, `rotate` |

Shape styling accepts `fill`, `stroke`, `strokeWidth`, `opacity`, `dash`, `marker`, `linecap`, and `linejoin`. Valid paints are profile tokens `paper`, `paper-2`, `ink`, `muted`, `soft`, `rule`, `rule-solid`, `accent`, `accent-tint`, and `link`; compact tints such as `ink@5` and `accent@12`; `none`, `transparent`, or `currentColor`; and `#rgb`, `#rrggbb`, `rgb()`, or `rgba()` colors. Markers are `default`, `accent`, or `link`.

Build errors identify the exact JSON path and report every invalid edge in one pass. Correct each field in its unique enclosing object rather than replacing a repeated scalar globally.

This vocabulary expresses every type without auto-layout: paths cover Sankey bands and loop arcs; lines and polygons cover fishbones, axes, radar, and Wardley maps; rectangles cover lanes, cards, tables, Gantt tasks, and treemaps; circles and ellipses cover scatter, Venn, and state marks.

## Review metadata

Nodes, edges, zones, and primitives accept two optional fields:

```json
{
  "code": ["src/session/store.ts:40-66", "src/session/types.ts:8-14"],
  "change": "modified"
}
```

`code` is one repository-relative `path:start-end` string or a list of them. The renderer emits a comma-separated `data-code` binding that Plannotator can open. `change` is `same`, `added`, `modified`, `removed`, or `projected` and emits `data-change`. Metadata does not choose the element's visual treatment; keep status text such as `PROJECTED` visible when the reader needs it. A projected element cannot carry a code binding.

## Contract

Required: `version`, `type`, `slug`, `title`, `description`. `nodes`, `edges`, `zones`, `primitives`, `legend`, `viewBox`, and semantic `tokens` overrides are optional. Coordinates use the 4px grid except data-derived marks. Keep the JSON as source; the build output supplies validation and the inspection image without requiring the generated HTML in context.