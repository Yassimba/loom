# Focused atlas views

Read after [consume.md](consume.md) has established the atlas and target state.

## Choose output

Resolve the diagram preference once per task: explicit user request, then
`<repo>/ai-docs/agents/diagrams.json`, then `~/.config/loom/diagrams.json`.
Each file is a JSON object with `"style": "polished"`, `"economical"`, or
`"inherit"`. Missing files and `inherit` defer to the next level; the final
default is `polished`. Report an unreadable or invalid preference briefly and
continue with the next level. Do not change the selected model or effort.

Both modes retrieve atlas facts, inspect selected diagrams as visual reference,
and verify relevant source. Keep the same scope, source links and PROJECTED
labels. This preference affects consumer views, not atlas creation or research.

- **Polished:** follow Focused output below, reusing atlas SVG/HTML geometry.
- **Economical:** invoke `mermaid-skill` to express the selected atlas context
  and task changes in Mermaid. Reuse an existing view unchanged if it already
  answers the question; otherwise use the atlas as reference without editing
  its SVG/HTML. Keep the system context and add views for distinct questions.
  Record the referenced atlas figure IDs beside each Mermaid view. Follow
  Mermaid output below for source links, rendering and delivery.

## Focused output

Use the actual atlas figure, in this order:

1. Embed its existing SVG unchanged when it answers the question.
2. Highlight the relevant SVG elements for a path-specific view. Keep the surrounding
   layout as context; multiple views can share one base figure.
3. Patch verified changed labels, bindings and edges in a local copy. Keep
   unchanged IDs, positions and routes. Explain extra facts beside the figure.
4. Draw only a missing detail that the closest figure cannot explain. Retain
   the atlas overview and connect the new detail to its named node or edge.

Use `figure`'s element IDs (or the legacy manifest fallback in `consume.md`) to
locate the corresponding `data-element-id` in the selected HTML. Copy the HTML
into the consumer's output directory and edit its inline SVG directly. Preserve
unchanged IDs, positions and routes; update labels and verified source bindings
on changed elements. Highlight a path with styling, without implying a code change.
If publishing an atlas refresh, update the semantic JSON sidecar to match.

Proposed elements remain unbound and visibly say PROJECTED. Show removed code
only in a diff illustration. Separate baseline-to-target changes from a
Blueprint's target-to-proposal changes; use two figures if they obscure one
another. If edits crowd the figure, split it into focused views and put detail
in adjacent prose. Use Mermaid for a specific remaining gap.

Run `python3 <diagram-design>/scripts/self_check.py local.html` and inspect the
changed rendering at its intended viewing size. Fix visible defects directly
in HTML. Export its SVG block for the existing document viewer and link back
to the atlas section. No JSON renderer or patch helper is required.

For old/removed code, include revision-qualified references beside the figure;
bind only ranges verified at the viewer's current repository state. Never make
old ranges look like current code. For cross-repository ranges, include the
repository ID and revision in the text rather than silently resolving against
the viewer's root repository.

## Mermaid output

Use this branch for economical views, missing figures or absent atlas coverage.
Invoke `mermaid-skill`. Inspect only the requested scope, draw the needed structure/flow/state/model, and keep
source references alongside the figure. Use Mermaid click links where the
viewer supports them; adjacent references are required regardless. PROJECTED
labels distinguish planned code. Do not invoke Diagram Design for this branch.

The consumer keeps its normal document: walkthrough, review, or plan. Put
provenance and new evidence there once. Reuse the normal exporter and inspect
each changed view once; unchanged atlas geometry needs no new drawing pass.
