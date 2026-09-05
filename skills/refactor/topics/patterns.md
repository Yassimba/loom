# Patterns review

What recurring collaboration needs a clearer shape?

Read the [review contract](../references/review-contract.md). Trace participants, responsibilities, and actual variation before naming a pattern.

## Recognize, align, dissolve

- **Recognize:** express a recurring collaboration as a pattern justified by actual variation or an invariant.
- **Align:** repair unclear responsibilities or contract mismatches in an existing pattern.
- **Dissolve:** replace machinery whose variation disappeared or whose concepts exceed its benefit with a direct function, native construct, or existing module.

For a candidate, consult the [pattern index](../references/python-patterns/README.md), then only its relevant implementation. Adapt the collaboration to the target language: the Python reference explains trade-offs, not a mandatory class structure.

## Patterns must pay rent

Apply the contract's total-cost comparison. A pattern earns its cost by owning an invariant, hiding complexity, isolating actual variation, or removing stable duplication.

Compare each candidate with the simplest direct implementation, including what callers must learn and maintain. A controller/service/repository chain forwarding one database call adds little; hiding a meaningful persistence contract can pay rent. A callable can express a strategy without a hierarchy.

## Completion

Account for recurring collaborations and existing pattern machinery. Each proposal names its move, rent, and comparison with the direct alternative while preserving the required contract. Keep near-patterns with reasons; cite broader ownership/state-model reframings as cross-topic structural findings.
