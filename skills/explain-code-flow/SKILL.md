---
name: explain-code-flow
description: "Explain a feature from system context to runtime detail, reusing atlas figures and facts. For a revision range, overlay the changed flow; use Mermaid when atlas coverage is missing."
---

# Explain Code Flow

Explain the live entry, the values that travel, and the final result. Reuse the
atlas's actual figures as focused views: keep their element IDs and layout,
then highlight the path or patch verified drift. Reusing its theme or viewer
alone is not diagram reuse. Inspect the local delta and missing facts.

## 1. Pin and retrieve

Name the feature boundary, entry point, and target revision (working tree by
default). For a diff, capture both revisions. Follow
[the shared atlas consumer procedure](../system-atlas/references/consume.md).
Confirm production code actually reaches the feature; show a composition gap
when it does not.

Done when retrieved topic IDs and pins, the target, relevant drift, and gaps
are explicit. Keep new evidence in the walkthrough, not a separate brief.

## 2. Explain the path

Select the smallest set of atlas figures that covers the requested questions.
Use the same figure with different highlights when it already contains the
needed detail. Cover context and the runtime spine; add another view only for
a distinct question. "More detail" first means a closer explanation of the
existing nodes, edges, values and branches.

Highlight the path directly in a copy of the atlas SVG. For a revision comparison, follow
[references/diagram-diff.md](references/diagram-diff.md). Missing figures use
`mermaid-skill`; they do not require Diagram Design.
Keep exact identifiers and source references beside each figure. A missing
revision pin requires source verification, not a new layout. Before drawing a
new figure, name the closest atlas figure and the specific detail it cannot
show with an overlay or an adjacent explanation.

Done when the entry-to-result path is complete and every new claim has source
evidence or an explicit uncertainty.

## 3. Deliver

Write `ai-docs/explanations/<feature>/walkthrough.md`: the whole path in one
sentence, baseline and target, selected figures with short explanations, then
the result. Link to the matching atlas topics/sections. Explain real values,
decisions, side effects, and failures, not only function names. Keep changed
items searchable as text. Source-check new/current references with
`scripts/check-anchors.py`; historical references are checked against their
named Git revisions.

Export selected atlas SVGs and Mermaid SVGs for embedding. Build
`walkthrough.html` with `scripts/build-html.py`; the existing
[annotation build](references/annotation-build.md) handles inlining. Inspect
changed/new figures once. Reply with the result and walkthrough path.

Done when the focused walkthrough is readable without opening the full atlas,
its references identify the right revisions, and it links back to deeper detail.
Record figure provenance: atlas figure ID/path and unchanged, overlaid, or new
(with the coverage gap). Use the existing exporter; author only the selection,
small overlay patches and prose. Reuse views locally; delegate only independent
missing-code investigation when delegation is authorized.
