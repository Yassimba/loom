---
name: explain-code-flow
description: "Explain a feature's runtime flow, data transformations, and input/output contracts as a visual narrative with Mermaid. Use when explaining code flow, creating a code walkthrough, or tracing an object's full lineage."
---

# Explain Code Flow

Explain a feature's data flow: inputs, transformations, outputs, side effects,
decisions, and failures. Ground every claim in actual code, with source references
and explicit unknowns. Before tracing, read
[references/repository-evidence.md](references/repository-evidence.md).

## Trace the story

1. Start at the runtime entry point or public API, not an arbitrary file.
2. Find the data spine: the important object or value that crosses stages.
3. Follow the happy path from input to final output before adding branches.
4. Trace failures, retries, and background or parallel processes as separate flows.
5. Choose the clearest psychological order: top-down, data-centric, request narrative,
   or problem-solution. Explain in the order a reader needs, not filesystem order.

For object lineage, trace creation, mutations, mappings, copies, storage, and
consumers. Show every identity or shape change and meaningful branch.

## Build narrative chunks

Write a section outline before writing prose or code. Each section should:

1. Motivate the problem it solves
2. Show a diagram
3. Present the relevant code through focused excerpts and verified `file:line` references
4. Explain non-obvious decisions

Treat each section as one conceptual chunk. Use one fenced `mermaid` block per
concept; do not cram everything into one diagram. Label edges with the data type or action being
performed, keep diagrams to about 10–15 nodes, and split larger concepts. Place each
diagram before the code it describes so the reader has a mental model first.

Do not reproduce complete files: source code remains the source of truth.

Read [references/content-brief-by-type.md](references/content-brief-by-type.md) to
choose each diagram and
[references/authoring-invariants.md](references/authoring-invariants.md) to keep it
focused. Prefer operations in nodes and data on arrows. Add sequence, class, ER,
state, or field-lineage views only when they answer a different question.

When comparing revisions, read
[references/diagram-diff.md](references/diagram-diff.md). Show additions with
`:::green`, removals with `:::red`, and changes with `:::orange` where supported.

Deliver the walkthrough in chat with fenced `mermaid` blocks. Do not generate SVG,
HTML, or image artifacts. The reader should be able to follow inputs to results
without reconstructing the call graph.
