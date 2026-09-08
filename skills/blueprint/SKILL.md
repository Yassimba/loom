---
name: blueprint
description: "Sketch a proposed code change visually with Mermaid before implementation."
---

# Blueprint

Make the proposed change clear enough to decide whether and how to build it:
the goal, current versus proposed behavior, affected code, key trade-offs,
and how success will be verified. Ground existing behavior in source;
distinguish proposals and unknowns from facts.

Prefer the smallest change that meets the goal. Reuse existing code and
interfaces before introducing new abstractions or dependencies.

Use fenced `mermaid` blocks; they render automatically in the user's session.
Show structure, data in and out flow, object lineage, interfaces, or lifecycles
with the diagram types that fit. Use complementary views for distinct questions.
Mark changes with `:::red` removed, `:::green` added, and `:::orange` changed
where supported.

Deliver directly in chat with short labels and brief prose. Include implementation
steps when useful. Create files only when requested; leave review tooling to
the user. Begin implementation only after explicit approval.
