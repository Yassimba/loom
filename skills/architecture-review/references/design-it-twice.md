# Design It Twice

Use only after the user selects a verified review finding and asks to compare exact interfaces. The review's evidence ledger and settled interview decisions are the design brief.

## Frame

Present:

- the knowledge that must become local;
- constraints every design must satisfy;
- dependency categories;
- required invariants, errors, ordering, configuration, and performance behavior;
- tests that must survive;
- a small illustrative usage sketch that is explicitly not a proposal.

## Parallel design

Discover executable read-only design agents. Launch exactly one top-level workflow with at least three parallel designers. Include the same domain vocabulary and module/interface/seam vocabulary in every prompt.

Give each designer a distinct constraint:

1. **Minimum interface:** 1–3 entry points; maximize leverage and hidden behavior.
2. **Common caller:** make the dominant use case trivial; expose exceptional behavior deliberately.
3. **Change isolation:** maximize locality for the verified change vectors.
4. **Adapter design:** include only when dependency analysis proves a real production/test or provider variation.

Each designer returns:

```text
Interface: operations plus invariants, ordering, errors, configuration, performance
Usage: one common and one failure example
Hidden implementation: knowledge absorbed behind the seam
Dependencies: category and justified adapters
Tests: observable interface-level cases
Trade-offs: leverage, locality, migration cost, thin spots
Deletion test: where complexity reappears if this module disappears
```

Designers must be radically different. Renaming the same methods is one design, not three.

## Compare

The parent verifies designs against source and settled constraints, then presents them sequentially. Compare:

- interface knowledge required by callers;
- depth and leverage;
- locality for each verified change vector;
- seam placement;
- adapter justification;
- test surface;
- migration and compatibility cost.

Recommend one design or a clearly explained hybrid. Reject speculative seams and any design that moves complexity into callers.

Do not edit production code until the user approves a design.
