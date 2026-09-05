# Flow difference

Record the comparison base and target separately from atlas pins. Follow the
[shared consumer procedure](../../system-atlas/references/consume.md) to account
for atlas drift before classifying the requested difference.

Inspect changed source and affected callers at both revisions. Use CodeGraph
when its index matches the required source; otherwise use targeted Git reads.
Classify additions, removals, and changed responsibilities/contracts/routes.

Follow the [shared output preference](../../system-atlas/references/overlays.md).
Reuse matching atlas figures and element IDs. Preserve layout and removed
elements’ original positions where possible. A separate unchanged figure is needed only when
it explains context the diff cannot show.

Use added (green +), removed (red dashed −), and modified (amber ~), with a
legend and text cues. Below each figure list the actual changes and their
revision-qualified source references. Only current verified ranges get current
viewer bindings; removed code points to the base revision in adjacent text.

Done when every colored element has a matching searchable text explanation
supported by the revision it describes.
