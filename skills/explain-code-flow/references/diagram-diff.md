# Diagram diff

A diff figure shows one diagram-design figure with the change between two revisions colored onto it. The current figure stays as it is; the diff variant sits beside it.

## Compute the change (step 2)

1. Anchor the diff to the same entry point as the current figure.
2. Prefer `calldiff diff <from> <to> --entry <entry> --locs`; it lists callees that appeared, disappeared, or moved with `file:line`. Without calldiff, spawn a second Explore worker for the same map at `from` and compare it with the `to` map edge by edge.
3. Classify every node and edge of the current figure:
   - **added** — exists at `to`, absent at `from`
   - **removed** — exists at `from`, absent at `to` (drawn back in so the reader sees what left)
   - **changed** — same identity, different callee, signature, field, or edge label
   - **unchanged** — everything else
4. Verify each added and removed item against source at both revisions; a moved callee is one removed edge plus one added edge, drawn as **changed** on the node.

## Draw the diff variant

Same layout, same nodes, same positions as the current figure, plus the removed nodes in their former position. Color carries the class; shape and a tag repeat it so the figure survives greyscale and colour-vision deficiency.

| Class | Fill | Stroke | Redundant cue |
| --- | --- | --- | --- |
| added | `#2e8b57 @ 0.12` | `#2e8b57`, width 1.5 | type tag `+` |
| removed | `paper` | `#c0392b`, dashed `4,3` | type tag `−`, name in `muted` |
| changed | `#d68a00 @ 0.12` | `#d68a00`, width 1.5 | type tag `~` |
| unchanged | as the current figure | `muted` | none |

Edges follow the same table: added edges solid green with the green arrowhead, removed edges dashed red, changed edges amber. Define three extra `<marker>` ids beside diagram-design's standard three: `arrow-added`, `arrow-removed`, `arrow-changed`.

This convention replaces diagram-design's focal-accent rule for the diff variant only: `accent` is unused so the three change colors read alone. A legend is mandatory here, placed in the editorial area below the SVG, one row per class present.

Rules from diagram-design that still apply: orthogonal connectors, label masks, no shared attach points, the pre-output checklist.

## Name and place

- File: `diagrams/<rung>-<name>.diff.html`, exported to `.diff.svg`.
- In `walkthrough.md`, the diff figure follows its current figure under **What changed**, with the caption `<name> — <from>..<to>`.
- Below the figure, list the changes as text so they are searchable: `+ Caller → Callee (file:line)`, `− Caller → Callee (file:line)`, `~ Node: what changed (file:line)`.

## Done means

Every colored element maps to one line in the text list, and every line in the list has source at the revision it names.
