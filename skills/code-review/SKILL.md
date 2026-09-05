---
name: code-review
description: "Review a change against repository standards and its originating spec, using atlas context and optional visual overlays."
---

# Code Review

Keep two independent axes: **Standards** (code quality and repository rules) and
**Spec** (the requested behavior). Atlas facts orient inspection; findings must
be supported by the actual diff and affected source.

## 1. Pin the comparison

Resolve the user's fixed point with `git rev-parse`; ask if none was supplied.
For committed work, capture `git diff <fixed-point>...HEAD` and
`git log <fixed-point>..HEAD --oneline`. Fail early on a bad ref or empty diff.
If the user requested working-tree review, include staged, unstaged, and relevant
untracked source and record that target explicitly.

Done when the comparison base and actual target are unambiguous.

## 2. Retrieve context

Find the spec in commit issue references, the user's supplied path, then
matching branch/feature documents in `ai-docs/` or `specs/`. Use
`ai-docs/agents/issue-tracker.md` for issue access. If missing, use Beads when
`br` is installed ([workflow](../loom/references/issue-tracker-beads.md)),
otherwise [local Markdown](../loom/references/issue-tracker-local.md).
Ask where the spec is if unresolved; an explicit absence skips the Spec axis.

Read applicable repository standards (AGENTS, CONTRIBUTING, coding standards).
Follow [the atlas consumer procedure](../system-atlas/references/consume.md)
once for relevant context, then inspect the diff and affected callers/contracts.
Keep atlas pins separate from the review comparison. New evidence stays beside
findings; no separately authored evidence packet.

Done when standards, spec availability, relevant atlas context, and local drift
are known.

## 3. Review both axes

Use separate workers when the scope benefits from parallel review; otherwise
review sequentially. Share the same retrieved context and diff command. Workers
read additional source only to resolve their own gaps.

**Standards:** cite each violated repository rule and affected hunk. The repo
overrides the baseline below; skip rules already enforced by tooling. Baseline
smells are judgment calls, labeled “possible …”, not hard violations.

| Smell | Look for / remedy |
| --- | --- |
| Mysterious Name | Unclear responsibility; rename honestly. |
| Duplicated Code | Repeated logic; share the existing implementation. |
| Feature Envy | Logic uses another object's data; move it nearer that data. |
| Data Clumps | Fields travel together; consider one meaningful type. |
| Primitive Obsession | Primitives obscure a domain concept; name the concept. |
| Repeated Switches | Repeated dispatch; centralize the decision. |
| Shotgun Surgery | One change scatters edits; consolidate its responsibility. |
| Divergent Change | Unrelated reasons change one module; separate responsibilities. |
| Speculative Generality | Unused abstraction/configuration; remove it. |
| Message Chains | Callers navigate internals; hide the traversal. |
| Middle Man | Empty delegation; remove it. |
| Refused Bequest | Subtype rejects inherited behavior; consider composition. |

Also assess modern type safety, reuse, DRY without premature abstraction,
architecture/SOLID and Demeter, integration, security, scalability, and performance
where relevant to the diff.

**Spec:** cite the requirement for each missing/partial behavior, unrequested
scope, or apparently implemented but incorrect behavior. If no spec exists,
report “no spec available.”

Done when both axes have evidence-backed findings or an explicit clean/skip result.

## 4. Report

Present **Standards** and **Spec** separately, at most 400 words each. Preserve
their findings without merging or reranking across axes. End with counts and
the worst issue within each axis.

If a figure materially explains a finding, follow the consumer procedure's
visual branch: reuse an atlas overlay, or invoke `mermaid-skill` for missing
coverage. Link back to the relevant atlas section. Leave the shared atlas unchanged.
