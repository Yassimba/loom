---
name: explain-code-flow
description: "Explain a feature's runtime flow, data transformations, and input/output contracts visually with Mermaid. Use when explaining code flow or tracing an object's full lineage."
---

# Explain Code Flow

Explain the feature's data in and out flow: inputs, transformations, outputs,
and side effects—including decisions and failures. Ground the explanation
in actual code, with source references and explicit unknowns.

For object lineage, trace an object from creation through mutations, mappings,
copies, storage, and consumers to its final outputs. Show where its identity or
shape changes, including branches—not just one call path.

Use fenced `mermaid` blocks; they render automatically in the user's session.
Prefer data-flow diagrams with operations in nodes and data on arrows. Add
complementary views when useful: sequence for interactions, class for interfaces
and input/output shapes, ER for data models, state for lifecycles, or field lineage
for derived values. Show changes with `:::red` removed, `:::green` added, and
`:::orange` changed where supported.

Deliver in chat with short labels and brief prose beside each diagram. The reader
should be able to trace inputs to results, not merely see a list of function names.
