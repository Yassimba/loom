---
name: whats-next
description: Preview the next actionable Beads task — its context and a sketch of the approach, claiming nothing. Use when the user asks what's next or what to work on next, or wants to size up the coming task before committing to it.
---

# What's Next

Read-only twin of the `next` skill: surface the same top pick from the Beads dependency graph, but report on it instead of claiming it. Every command this skill runs reads state; the graph is exactly as found when it finishes.

1. Confirm the repository contains `.beads/` and both `br` and `bv` are on `PATH`. If any prerequisite is missing, report it and stop.
2. Run `bv --robot-next`. Use only `bv --robot-*` commands; bare `bv` opens an interactive TUI.
3. Read the returned top pick fully: `br show <id> --json`. (Reads need no actor, so the launcher the `next` skill uses stays out of this.)
4. Skim the code the task touches — enough to ground the approach sketch, not to start the work.
5. Report three sections:
   - **Task** — ID, title, state, priority, and what it blocks or is blocked by, from the `bv` reasoning.
   - **Context** — why this task exists and why it is next, in plain terms a returning teammate would follow.
   - **Approach** — how the work would go if claimed: rough steps, the files and areas it would touch, the skills or workflow it would route through, and any open question worth settling first.
6. Close by offering to claim and start it via the `next` skill.

If `bv` returns no task, run `br ready --json`: an empty list is an empty frontier; epics only are a ready epic frontier; any non-epic is a `bv` discrepancy. Report which.

Done when the report covers task, context, and approach for the top pick — or names the stop condition — and the graph is untouched: no claim, no state change.
