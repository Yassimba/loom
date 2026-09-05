# Whole-repository OSS scan

Use within the assigned OSS review and [review contract](review-contract.md). Request missing assignments through the parent, which owns dispatch; keep one orchestration tree.

## 1. Survey

Map every in-scope source area to an assigned package/subsystem slice with approximate size, available dependencies, and explicit integration flows. Use the parent's snapshot and exclusions.

## 2. Inspect

Read each slice's implementations against existing helpers, dependencies, and relevant [catalog](oss-libraries.md) entries. Use an available clone detector; otherwise inspect duplication directly.

For each mechanism, record the contract's finding fields plus purpose, approximate owned code, candidates, semantic fit, and net reduction after adapters/migration. Account for every assigned source area, clone candidate, and cross-slice use. Classify upstream-vocabulary drift separately from behavior-preserving substitutions.

## 3. Synthesize

Group each finding once:

1. Underused existing dependencies, stdlib, or platform features.
2. New dependencies with demonstrated net benefit.
3. Bugs or semantic changes requiring explicit approval.
4. Kept with reasons.
5. Unresolved candidates and coverage gaps.

Retain the full contract, expose cross-slice prerequisites, and suggest implementation order by dependencies and risk. Estimate total reduction without double-counting overlapping alternatives. Return catalog candidates as the OSS topic specifies.

Discuss planned/empty modules separately only when the user requested architecture decisions; they are not code to refactor.
