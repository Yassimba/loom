---
name: blueprint
description: "Design non-trivial code changes before implementation as zoomable, code-bound Plannotator Markdown plans with a projected structural diff, data in/out flow, implementation ledger, and approval gate. Use before substantial implementation or when asked for a design or plan."
---

# Blueprint

Explain what **will change**. Existing elements are source-backed; proposed elements are visibly **PROJECTED**. The approved Markdown plan is the implementation contract.

The planning sequence is **PIN → MAP → PROJECT → DRAW → REVIEW → LOCK**. Production code begins after LOCK.

## 1. PIN the change

Create `ai-docs/blueprints/<slug>/brief.md` from the request, ticket, or specification. Name:

- outcome, acceptance criteria, constraints, non-goals
- live entry point and final effect
- **tracer**: the value, event, or entity whose journey changes
- repository baseline: `HEAD` plus working-tree state

Ask about ambiguity that changes system shape. Mark smaller uncertainties `ASSUMPTION`.

Done when the boundary has one entry, tracer, final effect, and checkable acceptance criteria.

## 2. MAP current evidence

Follow [`../explain-code-flow/references/repository-evidence.md`](../explain-code-flow/references/repository-evidence.md). Query `.codegraph/` once when present; otherwise give one relentless Explore worker that reference as its contract.

Map the live path end to end once: components, calls, concrete values in/out at every tracer hop, state changes, externals, affected files and symbols, and machinery the design preserves. Copy current anchors from `grep -n` or `sed -n`; projected code has no anchor.

Write the verified map to `evidence.json` as the shared packet for projection, every figure, review, and implementation. Later workers read this packet instead of repeating repository discovery.

Verify `brief.md` with `explain-code-flow/scripts/check-anchors.py` resolved from its skill directory.

Done when every current node and edge has verified source, the tracer has concrete input and output shapes, and `evidence.json` contains the complete shared map.

## 3. PROJECT the change

Write `changes.json`, the projected edit ledger and compact implementation handoff defined in [`references/guided-review.md`](references/guided-review.md). Give every addition, removal, and changed contract a stable id (`C1`, `C2`, …), reason, and verification.

Project current → proposed across nodes and edges:

- **added** — exists only in the proposal
- **removed** — exists now and leaves
- **changed** — same identity, changed responsibility, contract, state, or route
- **unchanged** — surrounding machinery deliberately preserved

Use [`../explain-code-flow/references/diagram-diff.md`](../explain-code-flow/references/diagram-diff.md) for the `+` / `−` / `~` palette and redundant cues. Label the result **PROJECTED**. Every colored element carries its ledger id as `data-change="C1"`; every id appears in the plan's searchable change list.

Done when every planned structural change has one ledger row and one projected disposition.

## 4. DRAW the explanation

Invoke `diagram-design`, then follow [`references/figure-selection.md`](references/figure-selection.md). Audit every current diagram type. The admitted figures must prove:

1. where the change sits
2. what changes
3. data in → transformations, state and side effects → outputs and failures

Use every selected `diagram-design` semantic pattern, profile, and type reference. Give one drawing worker `evidence.json`, `changes.json`, the figure selection, `explain-code-flow/scripts/draw.py`, `example-figure.py`, and `references/authoring-invariants.md`. The worker renders the full admitted set from the shared evidence packet; it does not remap the repository.

Keep drawing scripts, HTML previews, and PNGs in one temporary working directory. Run `explain-code-flow/scripts/check-figures.sh` there, inspect all PNGs together as one contact sheet, and copy only the final SVGs into the Blueprint's `diagrams/` directory.

Bind useful existing code with exact line ranges:

```html
<g data-code="src/session/store.ts:40-66">
```

Keep projected elements unbound and visibly `PROJECTED`. Keep `viewBox` on every SVG so Plannotator can maximize it. Fix every defect found in the contact sheet and rerun the affected checks.

Done when every DRAW verdict has one checked, retained SVG, every binding resolves, every projected node is labeled, and the required facts are covered.

## 5. REVIEW the plan

Load [`references/guided-review.md`](references/guided-review.md). Build the canonical `plan.md` with the intent, current boundary, projected diff, tracer spine, admitted design views, ordered implementation path, verification, risks, rollback, and untouched areas. Reference every admitted SVG once with the exact empty-body `plannotator-svg` directive.

Then run a prose gate over every authored artifact — `brief.md`, the `changes.json` strings, `plan.md`, and every figure label and callout — before the validator:

1. Invoke the `stop-the-slop` skill in **Improve** stance. These are reference documents: no AI patterns, every fact kept.
2. Check the result against the `i-have-adhd` rules: each overview leads with its point, lists are numbered and capped at five, and no sentence asks the reader to hold off-screen state.
3. Hold the register of `write-simply`: ASD-STE100 Simplified Technical English. Replace each jargon word with the plain term, or define it in bold at first use when the reviewer needs it to search the code.

A sentence that fails any check is fixed and re-checked before validation runs.

Run the Blueprint validator, then submit `plan.md` through Plannotator as that reference specifies. The reviewer can expand figures, reveal bound existing code, and annotate stable elements or the whole figure.

- **Revise** — address annotations, rebuild affected artifacts, validate, and resubmit the same plan path.
- **Rethink** — return to PIN with the rejected approach recorded.
- Closing without approval pauses implementation.

Done when the reviewer explicitly approves one valid plan revision.

## 6. LOCK the contract

After explicit Plannotator approval, run the validator's `--lock` command from [`references/guided-review.md`](references/guided-review.md). It preserves `approved-plan.md` and writes `approval.json` with the approved plan hash, repository baseline hash, `HEAD`, plan path, and timestamp.

Implementation may begin once both files exist. Implementation workers read the handoff in `changes.json`, `evidence.json`, and the named source files. They load `plan.md` or figures only when a listed unresolved risk points there. A later design change creates a new reviewed Blueprint revision rather than rewriting the approved contract.

Done when `approved-plan.md` matches the reviewed plan, `approval.json` records its reproducible baseline, and the compact handoff can start implementation without the full review package.

## After implementation

Load [`references/verify-built.md`](references/verify-built.md). Compare **PROMISED → BUILT**, dispose every ledger id and acceptance criterion, bind actual changed code, and reopen unexplained drift for review.
