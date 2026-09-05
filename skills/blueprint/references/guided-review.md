# Compact Markdown plan review

The author writes `plan.md` and `overlay.json`. Scripts generate the context
snapshot and rendered figures. Existing approved legacy packages remain valid;
new plans use version 2.

## Contract

`plan.md` contains these headings: Intent, Acceptance criteria, Changes,
Implementation, Verification, Risks. Changes use stable C1/C2 IDs with target,
current → proposed behavior, reason, and check. Put new source-backed facts in
the relevant section. Treat rollback/migrations under Implementation or Risks
when needed; record non-goals alongside Intent.

`overlay.json` records provenance and figure patches, not a second plan:

```json
{
  "version": 2,
  "target": {"head": "FULL_COMMIT_ID", "baselineSha256": "SOURCE_BASELINE_HASH", "workingTree": true},
  "atlas": {"path": "ai-docs/atlas", "topics": ["api.session"], "snapshot": "context.json"},
  "figures": [{"id": "session-write", "source": "diagrams/api/01-session.json",
    "patch": {"modify": ["store"]}, "output": "diagrams/session.svg"}]
}
```

For no atlas, use `"atlas": null`; figures authored with Mermaid use
`"mermaid": "diagrams/session.mmd"` instead of source/patch. Use `figures: []`
only when a purely textual change has no useful visual. This does not excuse
missing architecture or runtime explanation for substantial changes.

Capture `SOURCE_BASELINE_HASH` before inspection with `check-blueprint.py
<blueprint-directory> --repo-root <repo> --baseline`. It excludes the generated
Blueprint directory, so authoring the plan does not invalidate its baseline.
Validation rejects source changes since inspection; inspect that drift before
updating the target hash. Review HEAD and baseline are separate from atlas pins.

Generate `context.json` with `atlas.py freeze <atlas> <topic-ids...> --output
<blueprint>/context.json`. It preserves selected facts and pins; readers do not
need the live atlas after approval. Retain final SVGs and fallback Mermaid
sources under `diagrams/`. Temporary atlas overlay JSON/HTML/PNG files live
outside the Blueprint directory. Historical facts retain their revision;
current code bindings must be verified against the review target.

## Render and submit

Use each selected SVG once in the plan with the exact empty-body directive:

````markdown
```plannotator-svg path="ai-docs/blueprints/example/diagrams/session.svg"
```
````

Keep its viewBox and supported data-code/data-change metadata. Mermaid figures
carry adjacent source references and visible PROJECTED labels where relevant.

Run:

```bash
python3 <blueprint>/scripts/check-blueprint.py ai-docs/blueprints/<slug> --repo-root <repo>
```

Submit `plan.md` through the active Plannotator plan-submission tool; in Pi,
`plannotator_submit_plan(filePath="ai-docs/blueprints/<slug>/plan.md")`.
If unavailable, enter the integration's Plannotator plan mode. General
annotation does not submit a plan review.

Revise affected sections/figures on feedback and resubmit. Closing without
approval pauses implementation.

## Lock

After explicit approval, run the same validator with `--lock`. It preserves
`approved-plan.md` and writes `approval.json` with baseline identity and hashes
of the plan, overlay, context snapshot, and retained diagram sources/outputs.
Later atlas refreshes cannot change the approved package.

The baseline hash detects a changed worktree but does not reconstruct one.
If planning includes uncommitted code, preserve its patch/source separately
before implementation or record the resulting verification limitation.
Never claim a HEAD-only comparison reconstructs a dirty approved baseline.

Existing v1 artifact validation and lock records remain supported. A later
proposal creates a new directory/revision; never migrate an approved plan in place.
