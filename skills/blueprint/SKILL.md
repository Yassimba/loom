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
At minimum, show the structure, input and output flow, interface dependencies, and a code snippet of the main interfaces.

Add object lineage or lifecycle views when they make the proposal easier to assess.
Use the diagram types that fit. Use complementary views for distinct questions.

Mark upcoming changes with `:::red` for removed, `:::green` for added, and `:::orange` for changed.
