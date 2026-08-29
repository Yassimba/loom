# Figure selection

Read the `diagram-design` semantic-pattern and visual-type tables, then shortlist only the types that could clarify this change. Write `figure-selection.md` with one verdict per shortlisted type:

```markdown
| Type | Verdict | Figure | Value |
| --- | --- | --- | --- |
| Architecture | DRAW | diagrams/01-context.svg | Shows the affected boundary and unchanged neighbours. |
| State machine | SKIP | — | No state field, guarded transition, or terminal outcome changes. |
```

A verdict answers one question:

> Would omitting this figure leave the reviewer unable to evaluate an important part of the plan?

- **DRAW** names the planning question, misunderstanding prevented, nodes, and guide section. Its Figure cell is one repository-relative SVG path.
- **SKIP** names why the shortlisted type adds no value, or the stronger admitted figure that proves the same fact. Its Figure cell is `—`.

Start with these likely candidates instead of auditing the full catalogue:

| Planning question | First candidates |
| --- | --- |
| Where does the change sit? | architecture |
| What changes structurally? | architecture with a diff palette |
| How does data become output? | data flow, zoomed data flow |
| What controls behavior or ordering? | sequence, state machine, flowchart |
| In what order does implementation land? | dependency graph, gantt, story map |

## Required coverage

The admitted set must show:

1. **Where it sits** — entry, affected boundary, surrounding components, externals, final effect.
2. **What changes** — projected `+` / `−` / `~` nodes and edges, with unchanged context at equal weight.
3. **Data in → data out** — concrete shapes, transformations, decisions, state, side effects, failures, outputs.

When the change has a human-facing surface, also show how a person meets it. When the ledger has more than three changes, also show implementation order. One figure may cover several facts when splitting would duplicate it.

Done when each shortlisted type has one justified verdict, every DRAW points to one existing SVG, and the admitted figures cover the required facts.
