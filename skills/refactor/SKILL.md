---
name: refactor
description: Five-angle refactor review with before/after proposals, annotation approval, and verified implementation.
disable-model-invocation: true
---

# Refactor

## 1. Scope

Use the user's scope; otherwise staged/unstaged changes and non-ignored untracked source. With none, announce a whole-repository review before dispatch. Exclude generated, vendored, and build output unless requested.

Read project instructions, runtime versions, and validation commands. Record repository, branch, relevant base/head revisions, and initial worktree/index state. List the assigned files/subsystems and give all reviewers one read-only snapshot including dirty and untracked content; a commit alone is insufficient.

## 2. Review

Follow the host's subagent protocol and available-agent discovery. Dispatch one read-only reviewer per `topics/` file asynchronously. Split oversized assignments by named subsystem, assigning each file once per topic and explicitly covering integration flows. Bound concurrency; report budget-limited coverage.

Supply each reviewer the snapshot, assignment, project constraints, and absolute paths to its topic and the [review contract](references/review-contract.md), instructing it to read both. Assign a durable report destination returned through the host's artifact mechanism. Collect a contract-compliant report for every assignment.

The parent owns reconciliation, approval, and implementation. On dispatch/tooling failure, preserve partial reports, report the exact failure and run/repository/worktree/ref state, and resume only through the same protocol after resolving the blocker.

## 3. Reconcile

Check all reports against the snapshot. Merge duplicates under stable IDs (`R1`, …), retaining contributing topics. Record prerequisites; group incompatible remedies as alternatives and recommend one with a reason. Rank by benefit and risk. Separate behavior changes and pending investigations from implementation-ready refactors, as defined in the contract.

Account for every finding as a proposal, alternative, or reasoned dismissal; retain kept areas and coverage gaps. Use the `write-simply` register to render the full review contract in self-contained HTML at `ai-docs/refactors/<scope>-<timestamp>.html`. Include scope/snapshot and reconciled IDs, escape code as text, and show each behavior line directly below its title without expansion. Put step 4's approval rules at the top.

## 4. Approve

With no proposals, deliver coverage and finish. Otherwise invoke the `plannotator` skill, when the tool is available, to annotate the HTML with structured decision output; without it, provide the file path and request decisions in chat.

**On explicit approval or feedback submission, unannotated behavior-preserving proposals are approved**, subject to these exceptions:

| Case | Decision |
| --- | --- |
| Skip/reject | Excluded |
| Revise | Pending approval of the revised proposal |
| Ambiguous feedback | Affected proposals pending clarification |
| Incompatible alternatives | Explicit named choice required |
| Behavior change | Explicit approval of stated effects required |
| Skipped/pending prerequisite | Dependent proposal pending; offer an independent revision if useful |
| Dismissal, empty output, silence, timeout, or tool failure | No approval |

Global holds/rejections apply to the whole report. Record feedback and per-ID decisions; approval covers only the reviewed version. Proceed with any independent batch whose proposals are implementation-ready, compatible, and approved, including all prerequisites.

## 5. Implement and verify

Compare affected files and callers with the snapshot before writing. Re-review changed evidence and renew approval for materially changed proposals. Preserve unrelated edits and the existing index. Catalog edits follow the [separate authorization rule](references/oss-libraries.md#catalog-maintenance).

Dispatch a writer asynchronously with the approved IDs and full proposals. Use low thinking for mechanical work and deeper reasoning for structural/semantic work. Keep one writer per worktree; apply prerequisites first.

Baseline relevant checks before editing; add focused characterization checks for uncovered behavior at risk. After implementation, run project-required and proposal-specific checks. Compare the diff with the approved batch; return scope expansion or design drift for approval.

Account for every approved ID as implemented and verified or blocked with its partial diff and failed/unrun checks. Report the artifact path, completed/pending/skipped IDs, validation results, and residual risks.

Task / scope:
$ARGUMENTS
