# Verify promise against build

Load this reference only after implementation. `approved-plan.md`, `approval.json`, `evidence.json`, and `changes.json` are the immutable promise; built source is the evidence.

## 1. Pin the implementation range

Read `approval.json` and the compact handoff in `changes.json`. Its `head`, baseline hash, approved plan hash, plan path, and timestamp identify the approved state. Preserve the approved artifacts; write verification output beside them rather than overwriting them.

Capture the real implementation patch from the approved baseline to the built working tree. When the baseline included uncommitted source, restore or reconstruct that exact state before computing the range; report when this cannot be proven.

Done when the implementation range and any baseline limitation are explicit.

## 2. Remap built evidence

Repeat the shared map in `evidence.json` against built code. Regenerate every admitted figure from actual source. Use `calldiff diff <approval-head> --entry <entry> --maxDepth 2 --locs` where supported, but treat baseline working-tree differences recorded in `approval.json` separately from committed call-flow differences.

Every actual node, edge, binding, and anchor comes from built source.

Done when the built map reaches the same entry, tracer, and final effect as the approved projection.

## 3. Dispose every promise

For every `changes.json` id record exactly one disposition:

- **delivered** — built as approved
- **amended** — built differently and explicitly accepted
- **missing** — promised but absent
- **additional drift** — built but absent from the promise

Map every implementation step and acceptance criterion to the same record. A disposition names built files and anchors, evidence, and any reviewer decision.

Done when every approved id, implementation step, and acceptance criterion has one disposition and every unplanned structural change is listed as drift.

## 4. Reopen the comparison

Write a second Plannotator Markdown review with:

- **PROMISED → BUILT** summary
- approved projected figures followed by built figures
- the disposition ledger
- the real implementation patch
- code bindings to actual changed source

Import and review it with Plannotator. Fix unexplained surprises or obtain explicit acceptance. Keep approved and verification plans separate.

Done when every surprise is fixed or accepted and the built plan matches the final source.
