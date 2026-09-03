# Swimlane

**Best for:** cross-functional processes, RACI-style flows, vendor handoffs, multi-team shipping workflows.

## Layout conventions
- Horizontal lanes (or vertical columns) — one per actor/team. Label each lane in the left margin (or top) with a Geist Mono eyebrow.
- Lane dividers: 1px hairlines.
- Process steps are rectangles placed inside the lane of the actor performing them; arrows show flow.
- Handoffs (arrows crossing lane boundaries) are the most important edges — consider coral on the handoff that introduces the most coupling or latency.
- Don't force equal step count per lane; a lane with one step is fine.

## Renderer recipe

Use a diamond node with labeled outgoing edges when the process branches:

```json
{
  "nodes": [
    {"id": "remote", "label": "Remote fix?", "shape": "diamond", "x": 440, "y": 240, "width": 144, "height": 88, "role": "focal"},
    {"id": "resolve", "label": "Resolve remotely", "x": 640, "y": 248, "width": 144, "height": 72, "role": "step"},
    {"id": "bench", "label": "Bench repair", "x": 440, "y": 392, "width": 144, "height": 72, "role": "step"}
  ],
  "edges": [
    {"from": "remote:right", "to": "resolve:left", "tone": "accent", "label": {"text": "YES", "x": 612, "y": 274}},
    {"from": "remote:bottom", "to": "bench:top", "label": {"text": "NO", "x": 532, "y": 360}}
  ]
}
```

## Anti-patterns
- Lanes without labels.
- A step drawn across two lanes (pick one owner).
- Arrows that snake back and forth — reorder steps so the flow is mostly straight.

## Handwritten fallback

Load an example only when the compact renderer cannot express a required effect:

- `assets/example-swimlane.html` — minimal light
- `assets/example-swimlane-dark.html` — minimal dark
- `assets/example-swimlane-full.html` — full editorial
