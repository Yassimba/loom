# Content brief by diagram type

Lifted from archify's `guide` router. Each row is the whole content brief for that type:
the question the diagram answers and what it must include. Nothing about rendering.

| Type         | Question answered                                   | Must include                                                                                          |
| ------------ | --------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| architecture | What exists, who owns it, and how is it connected?  | 8–12 core components; one primary path; external dependencies; trust boundaries                       |
| workflow     | What are the steps, who owns each, where are gates? | lanes = responsibility or phase; columns = progression; happy path monotonic; exceptions routed outside |
| sequence     | In what order do calls happen?                      | callers and callees; request and return messages; fallback or error path; async side effects          |
| dataflow     | Where does data come from and go?                   | sources and assets; transform stages; classification or consent; stores and consumers                 |
| lifecycle    | What states exist and what moves between them?      | start and active states; event-labelled transitions; wait and retry states; all terminal outcomes     |

Use / avoid, per archify:

- architecture — use for onboarding, design reviews, repository orientation. Avoid when the audience needs exact call order, state transitions, or row-level lineage.
- sequence — avoid when the reader needs the landscape rather than the order.
- lifecycle — a card or note saying "retry" is not topology; a recoverable failure needs a real transition back to an active state.

Copy-ready prompt archify emits for architecture:

> Analyze this repository, then create a high-level architecture diagram. Show 8–12 core runtime
> components, one primary request or data path, external dependencies, ownership or trust
> boundaries, and put supporting detail in cards instead of adding more edges.

## Complexity budget (per figure)

From diagram-design §7; `draw.py` does not enforce these, the author does.

| Limit | Rule |
| --- | --- |
| Nodes | 9 (zone containers, start dots, terminal rings, and callouts do not count) |
| Arrows / transitions | 12 |
| Coral (focal) elements | 2 |
| Lifelines (sequence) | 5 |
| Fragments (sequence) | 1; 2 only when each is a single-region `opt`/`loop`, never nested |
| Classes (UML) | 7 |
| Entities (ER) | 8 |
| Callouts | 2 |

Over budget: keep the main path, turn side branches into a callout or a sublabel, and say in the report what was cut. Never cut the focal element the brief names or a terminal outcome; above nine real nodes it is two figures.
