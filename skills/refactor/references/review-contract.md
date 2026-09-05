# Review contract

Review the assigned snapshot read-only. Read implementations, relevant callers, and tests; apply the topic's question within scope and cite cross-topic findings for reconciliation.

## Findings

Return numbered findings, each starting with one concise line about user- or caller-visible behavior:

| Label | Content |
| --- | --- |
| **Behavior: unchanged** | Observable contract preserved |
| **Behavior: changed — explicit approval required** | Affected case: `current outcome → proposed outcome` |
| **Behavior: uncertain — pending investigation** | What remains unverified |

Example: **Behavior: changed — explicit approval required:** Invalid config uses defaults → startup fails with an error. Bug fixes count as behavior changes.

Then include:

- **Evidence:** snapshot, repository-relative file/line ranges, problem, and affected callers.
- **Change:** concrete remedy, design rationale, prerequisites, and incompatible alternatives.
- **Payoff:** complexity, ambiguity, or maintenance removed versus total cost across callers, implementation, tests, dependencies, interfaces, files, configuration, call hops, and concepts. Line deltas are supporting estimates.
- **Preservation:** evidence for the behavior line, covering relevant errors, ordering, serialization, and side effects.
- **Check:** named tests/commands or a specific proposed check detecting a broken preservation claim; distinguish run from proposed checks.
- **Risk:** compatibility constraints, unresolved assumptions, and required scope expansion.

Follow with **Before** (verbatim relevant source) and **After** (proposed replacement), each in source-language-tagged fenced blocks. Include changed call sites needed to judge the design; use labeled file blocks for structural changes. Mark omissions and illustrative sketches.

**Implementation-ready** means a concrete design, resolved behavior claims, and specified verification checks. Unresolved sketches or equivalence questions remain investigations. Readiness does not grant approval; the parent applies the skill's approval rules.

## Coverage

End with a compact table accounting for every assigned area against the topic's completion criterion: proposed, kept with reason, or unresolved. Include inspected files/boundaries/flows and mark unread areas or missing evidence as gaps.

No findings is valid: account for scope rather than meeting a suggestion quota. Return the report through the assigned artifact destination; the parent owns presentation and decisions.
