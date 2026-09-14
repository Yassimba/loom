---
name: blueprint
description: "Sketch a proposed code change visually with Mermaid before implementation."
---

# Blueprint

Design the proposed change, load the `ponytail` skill if available and reuse as much of the existing code and interfaces before adding abstractions or dependencies.

Run the `code-contracts` skill's obligation sweep while designing. Once the direction is approved, write each approved contract into its existing target declaration or `CONTRACTS` file before implementation begins; make contract-only edits and validate them with `cc-check format` and `cc-check list`. Do not create implementation scaffolding merely to host a contract. When the target code does not exist yet or an assumption still needs approval, leave the contract in the blueprint instead.

Include a `## Code contracts` section listing materialized contracts by path and ID, followed by unmaterialized proposals with their intended path, exact `@cc` text, and blocker. If no candidate survives the skill's violation-and-alternative test, say so and explain why. Completion: no approved contract remains only in the blueprint when a valid code location already exists.

Then present the change using mermaid diagrams, clear enough to decide whether and how to build it:
the goal, current versus proposed behavior, affected code, key trade-offs,
and how success will be verified. Ground existing behavior in source; distinguish
proposals and unknowns from facts.

Use fenced `mermaid` blocks; they render automatically in the user's session.
At minimum, show the structure, input and output flow, interface dependencies, code-contract status, and a code snippet of the main interfaces.

Add object lineage or lifecycle views when they make the proposal easier to assess.
Use the diagram types that fit. Use complementary views for distinct questions.

Mark upcoming changes with `:::red` for removed, `:::green` for added, and `:::orange` for changed.
