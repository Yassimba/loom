# Markdown plan review

The canonical review document is `plan.md`. Plannotator renders its repository-backed SVG directives in the ordinary Markdown plan review, where the reviewer can expand figures, inspect bound source, annotate stable figure elements, revise, and approve.

## Artifacts

```text
ai-docs/blueprints/<slug>/
├── brief.md
├── evidence.json
├── changes.json
├── figure-selection.md
├── plan.md
└── diagrams/
    ├── 01-context.svg
    └── ...
```

`evidence.json` is the verified repository map shared by projection, drawing, review, and implementation:

```json
{
  "entry": "src/session/api.ts:40-66",
  "finalEffect": "One deduplicated session is stored",
  "tracer": {
    "name": "session request",
    "input": "CreateSession",
    "output": "Session"
  },
  "hops": [
    {
      "source": "src/session/store.ts:40-66",
      "input": "CreateSession",
      "output": "Session",
      "stateChange": "Insert one session",
      "sideEffects": [],
      "failures": ["Duplicate identity"]
    }
  ]
}
```

`changes.json` is the projected edit ledger and compact implementation handoff:

```json
{
  "handoff": {
    "entry": "src/session/api.ts:createSession",
    "tracer": "session request",
    "finalEffect": "One deduplicated session is stored",
    "acceptanceCriteria": ["A duplicate request does not create a second session"],
    "unresolvedRisks": []
  },
  "changes": [
    {
      "id": "C1",
      "class": "changed",
      "target": "src/session/store.ts:createSession",
      "current": "Writes immediately",
      "projected": "Checks identity before writing",
      "reason": "Prevent duplicate sessions",
      "verification": "The duplicate-session acceptance example passes"
    }
  ]
}
```

Classes are `added`, `removed`, or `changed`. Every projected SVG element carries its ledger id as `data-change="C1"`. Every ledger id appears in the plan's searchable change list.

## SVG directives

Reference each admitted figure exactly once with this exact empty-body directive:

````markdown
```plannotator-svg path="ai-docs/blueprints/example/diagrams/01-context.svg"
```
````

The path is repository-relative, double-quoted, and ends in `.svg`. Do not add attributes or content inside the fence. Keep SVG bytes in their files; do not inline or snapshot them in `plan.md`.

Bind only existing source that helps the reviewer judge the plan. Every binding is repository-relative and line-bounded:

```html
<g data-code="src/session/store.ts:40-66" data-plannotator-anchor="session-store">
```

Several `data-code` values can be comma-separated; Plannotator opens the first one. Use `data-plannotator-anchor` for elements that need stable comments between plan revisions. An unmarked click creates a figure-level comment. Projected elements stay unbound and visibly say `PROJECTED`.

Keep `viewBox` on every SVG. Persist only final SVGs. Keep drawing scripts, HTML previews, PNGs, and contact sheets outside the repository.

## Plan shape

Write `plan.md` in this order:

1. Intent, acceptance criteria, and non-goals.
2. Current boundary.
3. Projected structural diff and searchable change ledger.
4. Data in → transformations, state, side effects, outputs, and failures.
5. Additional admitted design views.
6. Ordered implementation path.
7. Tests, migration, compatibility, rollback, risks, and untouched areas.
8. Approval decision and the contracts that approval locks.

Prose carries intent, evidence, trade-offs, and build order. Figures carry shape. Place each directive beside the prose it supports.

## Validate and submit

Validate the complete artifact:

```bash
python3 <blueprint-skill>/scripts/check-blueprint.py \
  ai-docs/blueprints/<slug> --repo-root <repo-root>
```

Submit the canonical plan through the active Plannotator plan-mode tool. In Pi:

```text
plannotator_submit_plan(filePath="ai-docs/blueprints/<slug>/plan.md")
```

If the active integration does not expose a plan-submission tool, enter its Plannotator plan mode first. Do not use the general `annotate` command as a fallback: repository SVG loading is restricted to the plan server.

The reviewer can expand figures, zoom, pan, fit, reveal source, annotate marked elements or the whole figure, approve, or deny with feedback.

- **Revise** — address annotations in the affected artifacts, validate, and resubmit the same `plan.md` path.
- **Rethink** — return to PIN and record the rejected approach.
- Closing without approval pauses implementation.

## Lock

After explicit approval, preserve the reviewed plan and repository baseline:

```bash
python3 <blueprint-skill>/scripts/check-blueprint.py \
  ai-docs/blueprints/<slug> --repo-root <repo-root> --lock
```

This writes `approved-plan.md` and `approval.json`. Do not run `--lock` before approval. Implementation starts only after both files exist.
