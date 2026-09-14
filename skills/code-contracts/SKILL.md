---
name: code-contracts
description: Use when code contains @cc or CONTRACTS, when the user asks to record obligations beside code, or when contract compliance must be verified.
---

# Code contracts

Record durable obligations beside the code they govern. Contracts are structured natural-language specifications, not runtime checks or formal proofs; keep tests, types, and validation that can enforce the same behavior mechanically.

## Workflow

1. **Discover.** Before designing, changing, or reviewing code, find every applicable contract on the target declaration, its enclosing declarations, ancestor `CONTRACTS` files, and relevant called symbols. Treat them as simultaneous obligations. Surface conflicts instead of choosing silently.
2. **Obligation sweep.** While planning or implementing, inspect the affected behavior for assumptions, requirements, invariants, input/output guarantees, error and fallback behavior, side effects, state transitions, security rules, and architectural boundaries worth preserving. Do not restrict the sweep to exceptional or high-risk behavior. Propose every concrete obligation that future changes must continue to satisfy.
3. **Place.** Put declaration-specific contracts in supported documentation comments or docstrings. Put directory-wide obligations in `CONTRACTS`. Use the narrowest boundary that governs the behavior. Once a blueprint direction is approved, materialize its contracts in existing code before handoff when possible; leave only contracts whose target code does not exist or whose assumptions remain unresolved for the implementation agent.
4. **Enforce.** Keep code, tests, and contract prose coherent. Update contracts with intentional behavior changes. Never weaken or remove a contract merely to make an implementation appear compliant; fix the mismatch or surface it.
5. **Verify.** When `loom contracts` supports the target language, run `loom contracts check` and confirm discoverability with `loom contracts list`; otherwise inspect manually and disclose the tooling limit. For `$code-contracts verify` or a contract review, read and follow [`references/verification.md`](references/verification.md).

## Writing contracts

Before writing or editing contracts, read [`references/format.md`](references/format.md).

Each contract carries one concrete obligation. Include decisive conditions, boundaries, missing-data behavior, errors, fallbacks, or effects when they determine compliance. Preserve intent across implementations: omit purpose-only documentation, rationale, algorithm narration, and accidental implementation details unless they impose a real constraint.

Keep a proposed contract only when a reviewer can identify both:

- a concrete change that would violate it; and
- an alternative implementation that would satisfy it.

When materializing a contract, set `owner` from the authenticated GitHub identity when available and ask rather than guess. A blueprint may omit unknown ownership metadata. Set `notify` only when the user explicitly requests violation notifications.

## Completion

Before completing work, account for every applicable contract and every durable obligation surfaced by the obligation sweep: represented by a satisfied or intentionally updated contract, rejected by the violation-and-alternative test, or surfaced as conflicting, obsolete, or impossible. Syntax validation alone does not establish semantic compliance.
