# Architecture review report contract

Read the complete `../assets/template.html` before generating HTML. Markdown is canonical; derive HTML from it rather than composing two reports separately.

## Paths

Default output:

- `docs/architecture-review/<YYYY-MM-DD>/architecture-review.md`
- `docs/architecture-review/<YYYY-MM-DD>/architecture-review.html`

Use a user-supplied directory when given.

## Required structure

```markdown
# Architecture Review

**Scope**: ...
**Abstraction level**: ...
**Date**: ...

## Executive Summary

## Context and Coverage

## Integration Overview

## Healthy or Intentional Coupling

## Issue AR-01: Short title

**Integration**: A → B
**Severity**: Critical | Significant | Monitor
**Recommendation confidence**: Strong | Worth exploring | Speculative

### Evidence
### Knowledge and Coupling
### Complexity and Change Cost
### Deep-Module Diagnosis
### Recommended Direction
### Before / After

## Accepted and Monitor

## Top Recommendation

---

_This analysis was performed using the [Balanced Coupling](https://coupling.dev) model by [Vlad Khononov](https://vladikk.com)._
```

## Executive summary

Use 3–5 sentences: what the system does, overall architecture status, number of actionable findings, top finding, and material coverage limit.

## Context and coverage

State domain classification, ownership, deployment/runtime topology, inspected paths, uninspected paths, assumptions, and unknowns. Never claim an entire-codebase review unless every first-level source area was covered.

## Integration overview

Use these linked headers exactly:

| Integration | [Strength](https://coupling.dev/posts/dimensions-of-coupling/integration-strength/) | [Distance](https://coupling.dev/posts/dimensions-of-coupling/distance/) | [Volatility](https://coupling.dev/posts/dimensions-of-coupling/volatility/) | [Balanced?](https://coupling.dev/posts/core-concepts/balance/) |
| --- | --- | --- | --- | --- |

Link strength values to the integration-strength page. Record essential and implementation volatility separately when they differ.

## Finding gate

A main issue must include:

- at least two exact source citations;
- observed fact separated from inference;
- abstraction level;
- shared knowledge and implicitness;
- strength with rationale;
- technical, ownership, deployment, and runtime distance;
- essential and implementation volatility with source/confidence;
- plausible change vector and concrete cascading edits;
- why distance makes recurrence costly;
- why doing nothing costs more than remediation;
- deletion-test result and test-surface effect;
- remediation direction and trade-off;
- disconfirming evidence or accepted uncertainty.

Unverified observations go in Accepted and Monitor, not the main issue list.

## Severity and confidence

Severity describes present architectural risk:

- **Critical:** high strength, high distance, high essential or implementation volatility, with evidenced recurring change cost.
- **Significant:** unbalanced coupling or low cohesion in a moderately/highly volatile area with a credible cascade.
- **Monitor:** verified imbalance neutralized by low volatility, low recurrence, or high remediation cost.

Recommendation confidence is separate:

- **Strong:** direct evidence, context known, simpler alternatives rejected.
- **Worth exploring:** evidence is sound but design constraints remain.
- **Speculative:** context or evidence gaps remain; never the top recommendation.

## Issue prose

### Evidence

List record IDs and `path:line` citations. State observations before interpretations.

### Knowledge and Coupling

Explain what knowledge crosses the seam, its strength, distance, volatility, and balance. Link the first use of each Balanced Coupling concept using `coupling-model.md`.

### Complexity and Change Cost

Name one concrete change and every likely cascading edit, coordination step, deployment interaction, and failure mode. Explain why the outcome exceeds local reasoning.

### Deep-Module Diagnosis

State interface knowledge, locality, deletion-test outcome, dependency category, and whether a real seam exists. A finding may have no deepening recommendation when co-location, a narrower contract, or acceptance is better.

### Recommended Direction

Choose reduce strength, reduce distance, deepen, or accept. Describe where knowledge should live and what behavior should become local. Include migration and testing trade-offs. Do not specify exact methods, types, or packages during review.

### Before / After

Use the same evidence in both formats:

- Markdown: Mermaid or a compact text diagram.
- HTML: equivalent diagram using `.comparison`, `.diagram-panel`, and `.diagram` from the template.

Labels alone must explain the change. Do not introduce claims that are absent from the issue prose.

## Healthy coupling

Include at least one evidenced integration whose strength and distance are balanced. This proves the review is not optimizing for maximal decoupling.

## Top recommendation

Name one issue, why it leads, the first decision required, and the smallest safe next step. A review may correctly return zero recommendations.

## HTML generation

Replace only `{{TITLE}}` and `{{CONTENT}}` in `../assets/template.html`.

Use:

- `.meta` for scope/date/coverage metadata;
- `.issue` around each issue;
- `.issue-meta` for integration/severity/confidence;
- `.severity` with `.severity-critical`, `.severity-significant`, or `.severity-minor`;
- `.comparison`, `.diagram-panel`, and `.diagram` for before/after visuals;
- `.top-recommendation` for the final recommendation;
- `.footer` for attribution.

HTML must preserve every report link. It must remain readable without network access; use no CDN dependencies.

## Parity check

Before completion, verify:

1. Same issue IDs, order, titles, severity, confidence, evidence, and recommendation direction.
2. Same integration rows, healthy examples, accepted items, scope, date, and attribution.
3. Every Markdown link has an HTML link.
4. No template placeholders or raw Markdown tokens remain in HTML.
5. HTML opens locally and diagrams remain understandable without scripts or network access.
