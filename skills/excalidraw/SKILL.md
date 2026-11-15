---
name: excalidraw
description: Excalidraw diagrams on the live canvas through the excalidraw MCP, with the visual type chosen from the diagram-design catalogue. Use when the user asks for a diagram in Excalidraw, or to change or redraw one on the canvas.
---

# Excalidraw

Draw on the live canvas (`http://127.0.0.1:3000`) with the `mcp__excalidraw__*` tools. `diagram-design` decides what to draw: its 45 visual types and their type references. The translation below and [`references/recipes.md`](references/recipes.md) decide how to draw it with Excalidraw elements.

Paths:

- `<dd>` = `<this skill's directory>/../diagram-design`
- `<history>` = `${XDG_STATE_HOME:-$HOME/.local/state}/excalidraw/history.tsv`, one line per diagram: date, type, subject.

## Steps

### 1. Pick the type

Choose the type whose **dominant axis** matches the reader's question. Content decides; habit does not.

1. Write the reader's question in one sentence. Name its axis: time, quantity, share, ownership, containment, topology, state, sequence, comparison, or cause.
2. Read `<dd>/SKILL.md` §3 (semantic-pattern table and visual-type guide). When behaviour carries the meaning, also read `<dd>/references/semantic-patterns.md` and choose one pattern.
3. Shortlist three types that answer the question from different axes. Read the first 15 lines of each `<dd>/references/type-<name>.md`. Keep a candidate only when the content meets its rules: a funnel needs counts that shrink stage by stage, a Sankey needs exactly 3 stages, a loop needs a last step that feeds the first.
4. Run `tail -n 12 <history>`. When two candidates fit equally, take the one used less recently. Architecture and Flowchart are fallback types: choose them when topology or branching is the dominant axis, and say why the other two lose.

When the user names a type, use it and go to step 2.

**Done when** you can state one line, and you give it to the user before you draw: `Type: <type> (pattern: <pattern or none>). Also considered: <A> (<why not>), <B> (<why not>).`

### 2. Load the grammar

Read the full `<dd>/references/type-<name>.md` and its row in [`references/recipes.md`](references/recipes.md). From the type reference, take the inputs contract, the budget, the placement rules, and the geometry formulas. Take colours, fonts, and shapes from the translation below.

Apply the complexity budget in `<dd>/SKILL.md` §7 and the type's own limits. Above the budget, draw an overview and offer the detail as a second diagram.

**Done when** you can list every element the diagram needs, each with its primitive and role, and the count is inside the budget.

### 3. Compute the coordinates

Put every origin and size on a 20px grid. A type is **parametric** when its reference or recipe gives formulas (angles, scales, squarified tiles, projections). For a parametric type, a short `python3` script computes the geometry and prints the element JSON.

**Done when** every element has x, y, width, and height (or `points`), and each number comes from the grid or the script.

### 4. Find space on the canvas

Call `describe_scene`. When the canvas holds elements, shift every x so the new diagram starts 200px to the right of their bounding box. The canvas keeps the user's work; clear it only when the user asks.

### 5. Draw

Send one `batch_create_elements` call in this order: zones, nodes, connectors, free text (title, zone labels, axis labels, legend). Give every node an `id`, so arrows bind to it and later edits find it. Then call `set_viewport` with `scrollToContent: true`.

**Done when** the call reports the element count you planned in step 2.

### 6. Verify

Call `get_canvas_screenshot` and look at the image. Fix with `update_element` or `delete_element`, then take a new screenshot.

**Done when** the screenshot shows all of these:

- each label on the lines you wrote, complete;
- every shape clear of the others, except a zone behind its contents;
- every connector running only between its own two ends;
- the type's grammar: a reader can name the type from the picture alone;
- accent within the limit in the roles table.

### 7. Log and report

```bash
mkdir -p "$(dirname <history>)" && printf '%s\t%s\t%s\n' "$(date +%F)" "<type>" "<subject>" >> <history>
```

Report the type line from step 1 and the canvas URL. Offer the two other candidates as redraws.

## Translation

### Roles to styles

`diagram-design` names roles. Use these Excalidraw values for them. Set `fillStyle: "solid"` on every filled shape. Use at most 4 fill colours in one diagram.

| Role | strokeColor | backgroundColor | Use |
|---|---|---|---|
| ink (default node) | `#1e1e1e` | `#ffffff` | most nodes |
| muted | `#868e96` | `#e9ecef` | default connectors, secondary nodes, axes, grid |
| accent | `#e8590c` | `#ffd8a8` | the 1 or 2 focal elements |
| link / external | `#1971c2` | `#a5d8ff` | HTTP/API calls, external systems, users |
| store / state | `#0c8599` | `#99e9f2` | databases, queues, stored state |
| service | `#9c36b5` | `#eebefa` | services, middleware |
| positive / negative | `#2f9e44` / `#e03131` | `#b2f2bb` / `#ffc9c9` | pass/fail, up/down, gain/loss |
| zone / boundary | `#868e96` | `#e9ecef`, `opacity: 40` | containers; `strokeStyle: "dashed"` for trust boundaries |

Text: title `fontSize: 28`; node names 16 to 20; small labels 14 minimum. Use `fontFamily: 3` (mono) for technical text only: ports, commands, IDs, SQL types, axis numbers.

### Primitives

| Need | Excalidraw element |
|---|---|
| node | `rectangle` with `text`. Width at least `max(160, longest line in characters × 10 + 40)`, height 60 for one line, 80 for two |
| decision | `diamond`, 120 × 120 minimum |
| state, set, point | `ellipse` (a 12 to 16px ellipse is a dot) |
| zone, lane, band | big `rectangle` drawn first, with its label as a free `text` element 12px inside its top-left corner |
| frame (Before/After, slide) | `frame` |
| axis, spine, lifeline, grid line | `line` with `points`; dashed for lifelines and grid |
| polygon (radar ring, pyramid tier, isometric face) | `line` with closed `points` (last point = first) plus `backgroundColor` |
| quantity band (Sankey, flow width) | `line` with `points`, `strokeWidth` in proportion to the amount, `opacity: 50` |
| tabular content (ER, schema, matrix) | a grid of `rectangle` cells, or one rectangle plus left-aligned free `text` |

### Connectors

Tested behaviour of this MCP server:

- **Bound arrows draw straight.** An arrow with `startElementId` and `endElementId` ignores `points` and `elbowed`, and follows the shapes when the user drags them. Put the two ends on the same x or the same y so the line is straight.
- **Routed arrows keep their path.** An arrow without bindings keeps its `points`, and `elbowed: true` rounds the corners. It stays put when the user drags a shape, so use it only where an aligned straight path cannot exist: a swimlane hand-off, a back-edge, a route around a zone.
- **Headless connectors are lines.** The tool rejects `endArrowhead: null`; draw a connector without a head as a `line`.
- **Dashed** = async, optional, return, or event. **Accent** = the one headline path.
- **Arrow labels** are 12 characters or fewer and sit at the midpoint. A bound arrow is 16px shorter than the gap between its shapes, and a label needs about `characters × 9 + 24`px of arrow, so leave a 160px gap where an arrow has a label. Label an arrow only when the layout does not already say it.

### Gotchas

- **Zone labels:** `text` on a large rectangle is centred in it and lands on the contents. Label zones with a free `text` element.
- **Symbols:** a non-Latin character (`→`, `✓`, `•`, CJK) makes that label fall back to a system font. Write words or ASCII (`->`, `-`, `x`).
- **User edits:** before you change a diagram, call `describe_scene` and work from what is there now. The user's edits win.
- **Saving and sharing:** `export_scene` with a `filePath` writes a `.excalidraw` file; `export_to_excalidraw_url` makes a share link. Use them when the user asks.
