## Issue triage with bv

bv is a graph-aware triage engine over the Beads issue graph (`.beads/issues.jsonl`). Use it for _what to work on_ (triage, priority, planning) instead of parsing JSONL or guessing graph traversal — it precomputes PageRank, betweenness, critical path, cycles, HITS, eigenvector, and k-core deterministically.

**Use ONLY `--robot-*` flags. Bare `bv` launches an interactive TUI that blocks the session.**

### Workflow: start with triage

```bash
bv --robot-triage          # THE entry point: quick_ref, ranked recommendations,
                           # quick_wins, blockers_to_clear, project_health, commands
bv --robot-next            # Minimal: single top pick + claim command
bv --robot-triage --brief  # Compact: only decision-relevant fields (~80% smaller)
bv --robot-triage --format toon   # Token-optimized output (or BV_OUTPUT_FORMAT=toon)
```

Count semantics: `actionable` = non-closed with no open blockers (ready now); `not_actionable` = non-closed but dependency-blocked; `not_closed == actionable + not_actionable`. `blocked_count` means status exactly `blocked`.

Before claiming, verify state with `br show <id> --json` or `br ready --json`. Only `quick_ref.top_picks` and non-empty `claim_command` fields represent claimable work — `recommendations` can include blocked or assigned items.

### Multi-agent claims

Every AI session carries its own Beads actor: `$BEADS_ACTOR`, injected as `--actor` by the
`~/scripts/br` wrapper. An `in_progress` bead's `assignee` therefore names the exact session
holding it — a Claude assignee is a bare session UUID the user can reopen with
`claude --resume <assignee>`; Codex/Pi assignees are `codex-<timestamp>` / `pi-<timestamp>`,
matched by timestamp in their session pickers (a resumed Codex/Pi session mints a fresh actor,
so re-stamp its old claims when handing work back).

- Work a bead only when it is unclaimed or its `assignee` equals your `$BEADS_ACTOR` — any
  other assignee (including bare `yassin`) is a live claim by another session; the user
  handing it to you explicitly is the one override.
- Claim atomically: `br update <id> --claim`. Upgrade bv's suggested `claim_command` (a bare
  `--status=in_progress`) to this form.
- Before claiming, confirm `echo $BEADS_ACTOR` prints a session-unique name; an empty actor
  means the environment is broken — repair it first.
- A claim goes stale after ~2h without an `updated_at` change (`br show <id> --json`) or when
  the user declares the session dead; reclaim with an audit comment saying why.

### Other commands

| Command                                             | Returns                                                                                                           |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `--robot-plan`                                      | Parallel execution tracks with `unblocks` lists                                                                   |
| `--robot-priority`                                  | Priority misalignment detection with confidence                                                                   |
| `--robot-insights`                                  | Full metrics: PageRank, betweenness, HITS, eigenvector, critical path, cycles, k-core, articulation points, slack |
| `--robot-label-health`                              | Per-label `health_level`, `velocity_score`, `staleness`, `blocked_count`                                          |
| `--robot-label-flow`                                | Cross-label dependency: `flow_matrix`, `bottleneck_labels`                                                        |
| `--robot-label-attention [--attention-limit=N]`     | Attention-ranked labels                                                                                           |
| `--robot-history`                                   | Bead-to-commit correlations                                                                                       |
| `--robot-diff --diff-since <ref>`                   | New/closed/modified issues, cycles introduced/resolved since ref                                                  |
| `--robot-burndown <sprint>`                         | Sprint burndown, scope changes, at-risk items                                                                     |
| `--robot-forecast <id\|all>`                        | Dependency-aware ETA predictions                                                                                  |
| `--robot-alerts`                                    | Stale issues, blocking cascades, priority mismatches                                                              |
| `--robot-suggest`                                   | Hygiene: duplicates, missing deps, cycle breaks                                                                   |
| `--robot-graph [--graph-format=json\|dot\|mermaid]` | Dependency graph export                                                                                           |
| `--export-graph <file.html>`                        | Interactive HTML visualization                                                                                    |

### Scoping and filtering

```bash
bv --robot-plan --label backend              # Scope to label's subgraph
bv --robot-insights --as-of HEAD~30          # Historical point-in-time
bv --recipe actionable --robot-plan          # Pre-filter: ready to work
bv --recipe high-impact --robot-triage       # Pre-filter: top PageRank
bv --robot-triage --robot-triage-by-track    # Group by parallel work streams
bv --robot-triage --robot-triage-by-label    # Group by domain
```

### Reading robot output

- All robot JSON includes `data_hash` (source fingerprint), `status` per metric (`computed|approx|timeout|skipped` + elapsed ms), and `as_of`/`as_of_commit` when using `--as-of`.
- Phase 1 metrics (degree, topo sort, density) are instant; Phase 2 (PageRank, betweenness, HITS, cycles) is async with a 500ms timeout — check `status`. Large graphs (>500 nodes) may approximate or skip metrics.
- Prefer `--robot-plan` over `--robot-insights` when speed matters; results are cached by data hash.

```bash
bv --robot-triage | jq '.quick_ref'                  # At-a-glance summary
bv --robot-triage | jq '.recommendations[0]'         # Top recommendation
bv --robot-plan | jq '.plan.summary.highest_impact'  # Best unblock target
bv --robot-insights | jq '.Cycles'                   # Circular deps (must fix!)
```
