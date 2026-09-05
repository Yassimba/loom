---
name: blueprint
description: "Plan substantial code changes using atlas context and projected views in SVG/HTML or Mermaid and Plannotator approval."
---

# Blueprint

Explain what will change. Existing facts come from the atlas plus verified local
changes; proposed elements visibly say PROJECTED. The approved plan is the
implementation contract.

## 1. Pin and inspect

Start `ai-docs/blueprints/<slug>/plan.md` with outcome, acceptance criteria,
constraints, affected surface, and repository target state. Name the runtime
entry and tracer when relevant. Resolve ambiguity that changes system shape.

Follow [the shared atlas consumer procedure](../system-atlas/references/consume.md).
Record reused topic IDs and baseline pins in `overlay.json`; put new facts and
source references directly in the plan. Inspect the relevant delta and gaps.

Done when the change boundary, target, reused context, and uncertainties are
explicit. No separate brief, evidence packet, or figure-selection report.

## 2. Project

Give each planned change a stable ID (C1, C2, …) in the plan's Changes section:
target, current → proposed behavior, reason, and verification. Include ordered
implementation steps, compatibility/migration needs, risks, and rollback where
applicable.

Select atlas figures by question, then follow
[the shared output preference](../system-atlas/references/overlays.md). For a comparison use
[the diff convention](../explain-code-flow/references/diagram-diff.md).
Separate atlas-to-current drift from current-to-proposal changes. Proposed
elements remain unbound and visibly PROJECTED.

Done when the plan explains every change and acceptance criterion, and its
figures reveal the important structure, runtime journey, or contracts.

## 3. Review and lock

Follow [references/guided-review.md](references/guided-review.md) for the compact
artifact contract, validation, Plannotator submission, and lock. Write plainly;
define unfamiliar terms once and remove repetition during the authoring pass.

Revise only affected plan sections and figures after feedback. Implementation
begins after explicit approval and successful lock. A later design change gets
a new reviewed revision rather than editing the approved contract.

Done when the approved plan, generated context/figures, and target baseline are
bound by the approval record.

## After implementation

Follow [references/verify-built.md](references/verify-built.md). Compare every
promised change and acceptance criterion with built source, explain drift, and
reuse unaffected diagrams.
