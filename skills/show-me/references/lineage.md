# Lineage

A lineage is a call graph that also carries the data: every hop shows what goes in and what comes out. Draw it in chat as one ASCII fence. Pick the shape by what the reader is tracing.

## Get the facts first

Name the entry and, for a change, the comparison in one line before drawing — silently picking is how the wrong graph gets drawn.

- Current shape: `codegraph explore <entry>` when the repo has a `.codegraph/` index; otherwise Grep the callers and callees and Read the hops. The source gives the data — Read each file where a hop's `args → return` is unclear.
- What changed: `calldiff diff [<ref> [<ref>]] --entry <entry> --maxDepth 2` when `calldiff` is on PATH (git-diff semantics: no refs → HEAD vs worktree). Otherwise diff the source by hand.
- Planned change: draw the current shape, then hand-write the intended `+`/`-` lines and say above the block that they are projected.

Label every line you could not verify from the source as `unverified`.

## 1. Call lineage — who calls whom

Boxes are functions; each arrow carries `args → return`; each box carries its `file:line`.

```text
┌─────────────────────────────┐
│ execute_plan  check_run.py:268 │
└──────────────┬──────────────┘
               │ (plan, connections) → Iterator[RunEvent]
               ▼
┌─────────────────────────────┐
│ _run_spec     check_run.py:292 │
└──────┬───────────────┬──────┘
       │ spec → Window  │ (spec, window) → list[CheckFinished]
       ▼               ▼
┌────────────────┐  ┌──────────────────────┐
│ _resolve_window│  │ _execute_checks :343 │
│ :297           │  └──────────────────────┘
└────────────────┘
```

## 2. Data lineage — one value through its stages

Boxes are stores and data objects; arrows are the functions that move or transform the value. Use this when the reader asks "where does X come from" or "what happens to X".

```text
 products.yaml ──load_project()──▶ Snapshot ──plan()──▶ RunPlanned
                                                          │
                                        execute_plan()    ▼
 duckdb ◀──_timed_query(sql)── CheckRunner ──▶ CheckFinished ──store.append()──▶ metadata.db
```

## 3. Change lineage — what a change did to the flow

The call lineage in a `diff` fence: `+` for hops that appeared, `-` for hops that disappeared, unchanged hops as context. Same for a planned change, marked projected above the block.

```diff
 accept_runs(data, state)
 ├─ data.to_request()            → CheckRunRequest
 ├─ try
+│  ├─ if not prepared.run_ids
+│  │  └─ _refusals(prepared)    prepared → str
 │  └─ prepared.execute()        → CheckRunOutcome
 ├─ except → from_host_error(error, mask)
-├─ if not prepared.run_ids
-│  └─ _refusals(prepared)
```

## Done means

Every line is verified, projected, or `unverified` by name, and one sentence under the fence answers the question that prompted the drawing.
