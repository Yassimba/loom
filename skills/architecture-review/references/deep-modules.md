# Deep-module reference

Use this only after coupling diagnosis identifies a verified change problem. It shapes remediation; it does not manufacture reasons to refactor.

## Depth

A deep module hides substantial behavior behind a small interface. Depth is leverage, not an implementation-lines ratio. A shallow module makes callers learn nearly as much as its implementation contains.

The interface includes operations, invariants, ordering, error modes, configuration, and performance expectations. Types alone are not the interface.

Apply three tests:

1. **Deletion test:** if deleting the module spreads complexity across callers, it earns its place. If complexity disappears or merely moves one hop, it is shallow.
2. **Interface test surface:** callers and tests should exercise the same seam. Tests that reach through it reveal the wrong module shape.
3. **Locality:** a likely change, bug, or rule should have one obvious implementation and verification location.

## Dependency categories

Classify dependencies before recommending a seam.

### In-process

Pure computation or memory state. Strongly related behavior can be co-located and tested directly. No adapter is needed.

### Local-substitutable

Filesystem, embedded databases, clocks, or other dependencies with realistic local stand-ins. Keep the seam internal and test the deep module with the stand-in.

### Remote but owned

An owned deployable across a network. Define a port only when independent deployment or a test adapter makes the variation real. Production and in-memory adapters can justify the seam.

### True external

A third-party system outside project control. Hide provider knowledge behind an injected interface when switching, testing, or supporting multiple providers is a plausible change vector.

## Seam discipline

One adapter means a hypothetical seam. Two justified adapters make it real. Do not expose internal seams because tests happen to use them. A deep module can keep private internal seams while presenting one external interface.

Choose the balancing move deliberately:

- **Reduce distance:** co-locate behavior that shares volatile functional knowledge.
- **Reduce strength:** publish a narrow contract where distance must remain high.
- **Increase depth:** absorb repeated ordering, invariants, and adaptation behind one interface.
- **Accept:** leave low-volatility or prohibitively expensive coupling alone.

Never recommend a new interface merely because dependency injection is possible.

## Testing direction

Recommend observable tests at the proposed module interface. When implementation is later refactored, replace shallow-module tests rather than layering a new test suite over all old internals. Surviving tests should describe behavior and remain stable while internal calls move.

During the review, state only:

- what knowledge should become local;
- where the seam should sit;
- what behavior the interface must hide;
- which dependency category applies;
- how the test surface improves;
- the cost and risk of moving it.

Do not specify exact methods or types until the user selects the candidate.
