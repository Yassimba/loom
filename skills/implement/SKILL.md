---
name: implement
description: "Implement a piece of work based on a spec or set of tickets."
disable-model-invocation: true
---

Implement the work supplied by the user. If they supplied no spec or ticket, take the next actionable ticket from the project's documented tracker and use it as the scope.

For Beads:

1. Confirm `.beads/`, `br`, and `bv` exist.
2. Run `bv --robot-next`; never run bare `bv`.
3. Verify the returned ticket is open, ready, and unassigned with `br show <id> --json`.
4. Claim it atomically with `br update <id> --claim`. If another actor wins, repeat from step 2.
5. Read the claimed ticket fully and treat its body as the request. If no actionable ticket exists, report that and stop.

Invoke `ponytail` and reuse as much existing code as possible.

Before coding, use `blueprint` to show the design and get approval. Skip it only for trivial changes such as a rename or one-line fix.

Use `tdd` where useful at pre-agreed seams. A ticket's Rules & Examples map is the scenario list: one test per example.

When implementation is complete, invoke `code-review`.
