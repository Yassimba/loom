---
name: ticket-overview
description: "Read-only Beads/BV ticket guidance. Use when the user asks what's next, wants an open-ticket roadmap, needs the complete issue graph, or asks what a set of tickets is building."
---

# Ticket Overview

Turn Beads structure into a concise product map. Use reads only; leave the graph unchanged. Never run bare `bv` because it opens the TUI.

## Choose a view

Use the requested view. If none is specified, ask:

1. **Next ticket** — preview the next actionable ticket.
2. **Roadmap** — map the current non-closed tickets.
3. **Complete graph** — map every open and closed ticket.

For named tickets, explain their one-hop dependency neighborhood. When combining Roadmap and Complete graph, show Roadmap first.

## Gather evidence

Confirm `.beads/` and `br` exist; views using BV also require `bv`. Report a missing prerequisite and stop.

### Next ticket

1. Run `bv --robot-next`.
2. Read its pick with `br show <id> --json`.
3. Skim the affected code enough to ground an approach without starting work.
4. If BV returns nothing, run `br ready --json`: report an empty list as an empty frontier, epics only as a ready epic frontier, and any non-epic as a BV discrepancy.

### Roadmap and Complete graph

Run:

```text
bv --robot-triage
bv --robot-graph --graph-format=json
bv --robot-plan
bv --robot-insights
br list --json
```

For Complete graph, also run `br list --status closed --json`. Roadmap excludes every closed ticket from counts, workstreams, hierarchies, and edges.

Use `br show <id> --json` only when list output truncates important detail or a central ticket's intent is unclear. For named tickets, start with `br show` for each ID and expand only when wider context is requested.

Check BV metric status before citing it. Keep literal `blocked` separate from dependency `not_actionable`. If the primary critical path is unavailable, use `advanced_insights.k_paths` and call it BV path analysis. Treat BV plan tracks as scheduling groups, not product workstreams.

## Interpret the graph

Derive product workstreams from ticket intent, acceptance criteria, parentage, and dependencies. Give each workstream an outcome. Ensure every in-scope ticket belongs to one primary workstream; show that assignment only when useful.

Explain central tickets as:

```text
why it exists -> what it owns -> what it unlocks
```

Keep relation types distinct:

- `parent-child`: membership; parent above child.
- `blocks`: raw BV edges normally point dependent to prerequisite. Confirm one against `br show`, then draw `prerequisite -> unlocked work`.
- `related`: association; keep outside execution order.

Keep ticket IDs visible and mark states consistently (`✓ closed`, `○ open`, `▶ in progress`, `■ blocked`).

## Write the result

Invoke `write-simply` and keep BV field names exact.

- **Next ticket:** **Task**, **Context**, and **Approach**; then offer to claim and start it via `implement`.
- **Roadmap:** purpose and open-ticket totals, main ASCII spine, workstreams, bottlenecks, independent tracks, and late or optional work.
- **Complete graph:** Roadmap plus the full parent hierarchy, ungrouped tickets, blocking DAG, and separate related edges. Include every node and blocking edge.
- **Named tickets:** brief product context, one-hop ASCII neighborhood, and the explanation for each ticket.

Report node totals and `blocks`, `parent-child`, and `related` counts for graph views. Label dependency-only BV edge counts as such. Prefer several small ASCII diagrams over one dense diagram. Attribute computed claims to BV and declared scope to the ticket.

Finish by confirming the selected coverage rule was met and tracker state was unchanged.
