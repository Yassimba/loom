# Tests review

Can the tests get simpler without losing fault detection?

Read the [review contract](../references/review-contract.md). Inspect production paths, the existing framework, and test commands alongside the assigned tests.

## Preserve the fault detector

Judge tests by detected production faults, not implementation size. Prefer stable use-case boundaries and observable results; assert internal call order only when it is contractual.

For every deletion or merge, map each original regression to a surviving assertion/property or explain why its protection is obsolete. Retain edge-case examples and past regressions unless broader coverage demonstrably subsumes them.

Replace mock machinery with real boundaries or small controlled fakes where fault detection becomes clearer. Preserve isolation from uncontrolled networks, clocks, randomness, and shared state; a higher-level test must still catch the original fault.

## Compress setup and cases

Use existing fixtures, helpers, and parametrization to remove repeated intent while keeping scenarios visible. Keep fixtures local unless several modules share their lifecycle.

A property-based proposal specifies its invariant/round trip/relationship, input domain and edge cases, independent oracle, replaced examples, and retained regressions. Prefer installed tools; add Hypothesis, fast-check, proptest, jqwik, or another ecosystem tool only when input space and fault detection justify it. Keep short case tables when clearer. Stateful testing requires a real transition model and sequence-dependent faults.

## Completion

Account for all assigned test areas and fault mappings. Mark lost coverage, uncontrolled nondeterminism, and unproven oracles unresolved. Include estimated per-module line deltas as supporting evidence, not the success criterion.
