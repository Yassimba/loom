# Nuclear structural review

Can changing ownership or representation eliminate whole categories of complexity?

Read the [review contract](../references/review-contract.md). Be ambitious about **code judo**: use an existing constraint, relationship, or owner to eliminate branches, modes, synchronization, or layers. Search beyond changed lines; state proposed edits and scope expansion explicitly.

## Trace and reframe

Trace each meaningful flow's inputs, state ownership, derived data, side effects, and callers. Find repeated decisions, stored derivable facts, and responsibilities that belong together. Explore:

- deriving indexes/views from authoritative relationships instead of synchronizing duplicate state;
- moving invariants to their owner to remove caller guards and special cases;
- representing legal states to eliminate flags and fallbacks;
- reusing canonical implementations instead of synchronizing parallel helpers;
- deleting pass-through layers or replacing generic machinery with the actual use case;
- separating independent responsibilities to reduce coupling and localize changes.

Compare the whole affected flow: show disappearing concepts and remaining responsibilities. Relocated conditionals must buy ownership or locality.

## Structural evidence

Crossing 1,000 lines triggers a cohesion check, not an automatic split. Base extraction on independent responsibilities or coupled edits; keep cohesive files when splitting increases navigation or cross-file knowledge.

For concurrency or transaction changes, check ordering, cancellation, failure recovery, and partial-update semantics before claiming equivalence. Apply the contract's behavior classification and readiness rules to any uncertainty.

Focus on ownership, representation, and whole-flow simplification; cite local type/pattern/test/library opportunities as cross-topic evidence.

## Completion

Account for meaningful flows, including high-risk areas kept with reasons. Each proposal identifies the structural cause, reframed flow, affected callers, and removed complexity. Rank structural regressions and high-benefit simplifications above local polish.
