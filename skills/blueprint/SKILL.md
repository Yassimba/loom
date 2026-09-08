---
name: blueprint
description: "Sketch a proposed code change visually with Mermaid before implementation."
---

# Blueprint

Design the proposed change, load the `ponytail` skill if available and reuse as much of the existing code and interfaces before adding abstractions or dependencies.

Then present the change using mermaid diagrams, clear enough to decide whether and how to build it:
the goal, current versus proposed behavior, affected code, key trade-offs,
and how success will be verified. Ground existing behavior in source; distinguish
proposals and unknowns from facts.

Use fenced `mermaid` blocks; they render automatically in the user's session.
At least show structure and data in and out flow and interfaces

But also object lineage, interfaces, or lifecycles and more if that makes it easier to understand and decide if this is how it should work.
Use the diagram types that fit and use multiple diagram (types) and complementary views for distinct questions.

Mark (upcomming) changes with `:::red` for removed, `:::green` for added, and `:::orange` for changed.
