# OSS review

What machinery can we stop owning?

Read the [review contract](../references/review-contract.md) and [library catalog](../references/oss-libraries.md), loading relevant ecosystem entries only. For whole-repository scope, follow the [OSS scan](../references/oss-scan.md).

## 1. Inventory

Read assigned packages' manifests and lockfiles. Identify declared dependencies, installed versions where available, runtime/platform constraints, canonical helpers, and applicable catalog sections.

## 2. Compare replacements

Apply the catalog's selection order to each custom mechanism. Compare current semantics with the replacement's version-specific documentation or implementation: inputs, outputs, errors, ordering, cancellation, resource lifecycle, and deployment support.

Count adapters and migration work against deleted code. For new dependencies, check maintenance, license compatibility, security, transitive weight, and runtime cost. Record unknowns.

Recommend contract-preserving swaps with lower total ownership cost. Keep custom code when domain semantics, error recovery, or size favor it; classify semantic mismatches under the review contract. Account for every inspected mechanism as proposed, kept with reason, or unresolved.

## 3. Catalog candidates

Report recommended packages missing from the catalog as `package — machinery it retires`, with necessary version constraints. Return candidates only; catalog edits follow the catalog's separate authorization rule.
