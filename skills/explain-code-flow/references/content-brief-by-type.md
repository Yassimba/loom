# Figure content contracts

Each figure answers one question. Type grammar and budgets constrain content; they do not choose the figure.

| Type | Question | Required content |
| --- | --- | --- |
| Architecture | What exists and connects? | exact composition seam at the live entry, primary path, final effect, core components, externals, real boundaries |
| Workflow | What happens, who owns it, where are gates? | lanes = owner/phase; columns = progress; monotonic happy path; exceptions outside corridor |
| Sequence | In what order do calls happen? | callers/callees, requests/returns, fallback or error, every externally visible side effect |
| Data flow | Where does each value go and change shape? | sources, transformations, custody/classification, stores, consumers |
| Lifecycle | What states exist and what moves between them? | start/active states, every direct event+guard transition, waits/retries, every terminal outcome |
| ER | How do domain entities relate? | verified entities, key fields, cardinality |
| Database schema | What is physically persisted? | real tables, SQL types/constraints, indexes, foreign keys |
| UML class | How do central types compose and implement contracts? | classes/protocols, selected members, inheritance/composition |

Use Architecture for orientation, not exact call order or state transitions. Sequence is for order, not landscape. A recoverable lifecycle failure needs a real transition back to an active state, not a “retry” note. Never turn one atomic event into a chain of invented intermediate lifecycle transitions.

## Complexity budget

| Limit | Maximum |
| --- | ---: |
| Nodes | 9, excluding containers/start/terminal/callouts |
| Arrows or transitions | 12 |
| Focal elements | 2 |
| Sequence lifelines | 5 |
| Sequence fragments | 1; two only for separate single-region `opt`/`loop`, never nested |
| UML classes | 7 |
| ER entities | 8 |
| Callouts | 2 |

Over budget: retain the main path, focal element, and terminal outcomes; aggregate side detail or split into figures with distinct facts. Never silently cut meaning to pass a check.