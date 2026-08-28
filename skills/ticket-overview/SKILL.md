---
name: ticket-overview
description: "Beads/BV ticket map with ASCII dependencies and product context. Use when the user asks for: (1) the complete issue graph, (2) roadmap workstreams and execution leverage, or (3) what a set of tickets is building."
---

# Ticket Overview

Turn tracker structure into a product story. Use `br list`, `br show`, and `bv --robot-*` reads only. Leave the graph unchanged.

## 1. Set the scope

Choose one mode from the request:

- **Complete graph:** Account for every node and edge.
- **Roadmap:** Compress the graph around the system, workstreams, main dependency spine, and leverage.
- **Ticket set:** Explain the named tickets and their direct dependency neighborhood. Expand to the full graph only when the user asks how they fit into the roadmap.

When the request combines complete and contextual views, write the roadmap first and the exhaustive graph second.

Done when the evidence boundary and output depth are explicit.

## 2. Collect current evidence

For a project-wide mode:

1. Confirm that `.beads/` exists and that `br` and `bv` are on `PATH`. Report a missing prerequisite and stop.
2. Start with `bv --robot-triage`. Use only `--robot-*` BV commands because bare `bv` opens the TUI.
3. Read structure with `bv --robot-graph --graph-format=json`.
4. Read scheduling with `bv --robot-plan` and leverage with `bv --robot-insights`.
5. Read active intent with `br list --json` and closed foundation with `br list --status closed --json`. Project the fields needed for the overview before loading long descriptions.

Report graph size as node count plus a breakdown of `blocks`, `parent-child`, and `related` records. Label a BV dependency-only edge count as such instead of comparing it with the sum of all relation types.

For a ticket set, require only `.beads/` and `br`, then start with `br show <id> --json` for every named ticket. The named tickets' dependency and dependent entries define the one-hop boundary. Their embedded IDs, titles, states, and edge types are enough unless the explanation needs a neighbor's full intent. Check for `bv` and use the project-wide commands only when the requested context needs graph metrics or the wider roadmap.

Read `br show <id> --json` when list output is truncated or when an epic, bottleneck, path ticket, ambiguous title, or acceptance criterion needs its full scope.

Check every BV metric's status. Distinguish literal `blocked` status from dependency-blocked work reported as `not_actionable`. When the primary critical-path metric is absent or skipped, use `advanced_insights.k_paths` and call it BV path analysis. Treat BV plan tracks as scheduling groups; derive product workstreams from ticket intent.

Done when every reported count, relationship, status, and scope statement has current tracker evidence.

## 3. Build the product story

Derive workstreams from descriptions, acceptance criteria, parentage, and dependencies. Use labels and title prefixes as supporting evidence. Give each workstream one outcome that states what becomes possible when it lands.

Assign every active ticket in scope to one primary workstream as an internal coverage check; print the assignment only when it helps the reader. Mention secondary connections without counting the ticket twice. Explain each central ticket as:

```text
why it exists -> what responsibility it owns -> what it unlocks
```

Group small independent defects by the product surface they affect. Explain the closed foundation as capabilities already available.

Done when a returning teammate can state what the project is building and why each in-scope ticket belongs.

## 4. Normalize graph language

Keep the edge types separate:

- `parent-child` means program membership. Render the parent above its children.
- A raw `blocks` edge normally points from dependent to prerequisite. Confirm one raw edge against the same ticket's `dependencies` entry from `br show`, then render `prerequisite -> work unlocked`.
- `related` means association. Render it outside the execution order.

Mark state consistently, for example `✓ closed`, `○ open`, `▶ in progress`, and `■ blocked`. Keep exact ticket IDs visible. A dense diagram may use short labels when each label includes its ID or has a nearby ID legend.

Done when membership, association, and execution order are unambiguous, and every drawn arrow has the stated direction.

## 5. Write the map

Invoke the `writing-clearly-and-concisely` skill (via the Skill tool). Apply its register to all prose while keeping ticket IDs, BV field names, and graph terms exact.

For a roadmap, include:

1. the system purpose and current graph totals
2. one ASCII main spine
3. one section per product workstream, with a small ASCII flow where it clarifies order
4. completed foundation, active bottlenecks, independent tracks, and late or optional work

For a ticket set, include a short product context, its ASCII dependency neighborhood, and the `why -> responsibility -> unlock` explanation for each ticket.

For a complete graph, add the full parent hierarchy, ungrouped tickets, full blocking DAG, and separate related edges after the roadmap. Every node must appear in the hierarchy or ungrouped section. Every blocking edge must appear in the DAG; compact repeated fan-in and fan-out while retaining every ID.

Use several small ASCII diagrams with one job each. State computed claims as “BV ranks” and declared scope as “the ticket describes.” Limit claiming advice to requests about what to do next.

Done when the selected mode meets its coverage rule, the prose adds purpose beyond ticket titles, and the final response confirms that tracker state did not change.
