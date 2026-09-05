# Flow difference

Record the comparison base and target separately from atlas pins. Follow the
[shared consumer procedure](../../system-atlas/references/consume.md) to account
for atlas drift before classifying the requested difference.

Inspect changed source and affected callers at both revisions. Use CodeGraph
when its index matches the required source; otherwise use targeted Git reads.
Classify additions, removals, and changed responsibilities/contracts/routes.

Reuse matching atlas figures and their element IDs. Edit existing edges and nodes
directly in a copy of the HTML/SVG; preserve layout where possible. Draw removed
elements in their original positions when available. Use Mermaid when no
suitable atlas figure exists. A separate unchanged figure is needed only when
it explains context the diff cannot show.

Use added (green +), removed (red dashed −), and modified (amber ~), with a
legend and text cues. Below each figure list the actual changes and their
revision-qualified source references. Only current verified ranges get current
viewer bindings; removed code points to the base revision in adjacent text.

Done when every colored element has a matching searchable text explanation
supported by the revision it describes.
