---
name: ticket-overview
description: "Read-only Beads/BV ticket guidance. Use when the user asks what's next, wants an open-ticket roadmap, needs the complete issue graph, or asks what a set of tickets is building."
---

# Ticket Overview

Turn Beads structure into a concise product map. Use reads only; leave the graph unchanged. Never run bare `bv` because it opens the TUI.

## Choose a view

Use the requested view. If none is specified, ask:

1. **Next ticket** — preview the next actionable ticket.
2. **Roadmap** — rank the current open tickets by business value.
3. **Complete graph** — map every open and closed ticket.

For named tickets, explain their one-hop dependency neighborhood. When combining Roadmap and Complete graph, show Roadmap first.

## Gather evidence

Confirm `.beads/` and `br` exist; views using BV also require `bv`. Report a missing prerequisite and stop. A git worktree has no `.beads/` of its own: take the directory from `br where` and export it as `BEADS_DIR` for every `bv` and `br` call.

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
br list --json
```

For Complete graph, also run `bv --robot-plan`, `bv --robot-insights`, and `br list --status closed --json`. Roadmap excludes every closed ticket from the ranking, counts, and edges.

Use `br show <id> --json` only when list output truncates important detail or a central ticket's intent is unclear. For named tickets, start with `br show` for each ID and expand only when wider context is requested.

Check BV metric status before citing it. Keep literal `blocked` separate from dependency `not_actionable`. If the primary critical path is unavailable, use `advanced_insights.k_paths` and call it BV path analysis. Treat BV plan tracks as scheduling groups, not product workstreams.

## Interpret the graph

Derive product workstreams from ticket intent, acceptance criteria, parentage, and dependencies. Give each workstream an outcome. Ensure every in-scope ticket belongs to one primary workstream; show that assignment only when useful.

For Roadmap, rank every open non-epic ticket by business value: how much a user of the product gains when the ticket ships. Use ticket intent first, then priority, then how much work the ticket unlocks. Group the ranking into tiers by feature area, most useful tier first. The ranking is your judgment; say so, and keep it apart from BV scores.

For Complete graph and named tickets, explain central tickets as:

```text
why it exists -> what it owns -> what it unlocks
```

Keep relation types distinct:

- `parent-child`: membership; parent above child.
- `blocks`: raw BV edges normally point dependent to prerequisite. Confirm one against `br show`, then draw `prerequisite -> unlocked work`.
- `related`: association; keep outside execution order.

Keep ticket IDs visible and mark states consistently (`✓ closed`, `○ open`, `▶ in progress`, `■ blocked`). Render relationships as fenced `mermaid` flowcharts, and Complete graph phases as a fenced `mermaid` timeline, so both display in chat. In flowcharts, use solid arrows for `blocks`, hierarchy edges for `parent-child`, and dotted lines for `related`; label each relation when its meaning is not obvious.

## Write the result

Invoke `write-simply` and keep BV field names exact.

- **Next ticket:** **Task**, **Context**, and **Approach**; then offer to claim and start it via `implement`.
- **Roadmap:** one numbered list that runs through all tiers, one line per ticket: full ticket ID, then the full title. Put a short tier heading above each group and mark a ticket that is in progress. Above the list, give one line with the open-ticket totals and the epics left out. Below the list, add a **Dependencies** section only when open tickets have `blocks` edges between them: small Mermaid flowcharts of `prerequisite -> unlocked work`, then the tickets that unlock the most. Give workstream tables, timelines, and BV track or path analysis only when the user asks for them.
- **Complete graph:** the Roadmap list, then a Mermaid timeline of workstream phases and Mermaid views of the full parent hierarchy, ungrouped tickets, blocking DAG, and separate related edges. Include every node and blocking edge.
- **Named tickets:** brief product context, one-hop Mermaid neighborhood, and the explanation for each ticket.

Report node totals and `blocks`, `parent-child`, and `related` counts for Complete graph and named tickets. Label dependency-only BV edge counts as such. Keep the timeline phase-level; use several small flowcharts rather than one dense dependency graph. Attribute computed claims to BV and declared scope to the ticket.

Finish by confirming the selected coverage rule was met and tracker state was unchanged.
