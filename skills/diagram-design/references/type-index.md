# Visual type index

Load this file only when the requested type is ambiguous. Once selected, load exactly one `type-*.md` reference. When behavior, state, enforcement, or risk is the point, also load `semantic-patterns.md`.

## Semantic routes

| Trigger | Pattern → type |
| --- | --- |
| Fan-in, queue depth, finite capacity, bottleneck | Fan-in queue / bottleneck → Data flow |
| Repeated Question / Input / Governance / Output slots | Stage framework with semantic slots → Process |
| Loose input becomes a structured durable artifact | Unstructured input → structured artifact → Data flow |
| Two policy traces need pass/fail/skipped and first divergence | Paired policy-evaluation traces → Flowchart |
| Trust boundaries and permitted/forbidden paths | Secure paved road → Architecture |
| Controls grouped by enforcement location | Governance / control catalog → Layer stack |
| Defenses compensate for gaps and residual risk propagates | Compensating security layers → Layer stack |

## Type routes

| Show | Type | Reference |
| --- | --- | --- |
| System components and connections | Architecture | `type-architecture.md` |
| Legacy landscape grouped by phase or department | IT current-state | `type-it-state.md` |
| Decision branches | Flowchart | `type-flowchart.md` |
| Time-ordered messages between actors | Sequence | `type-sequence.md` |
| States, transitions, guards | State machine | `type-state.md` |
| Entities, fields, relationships | ER / data model | `type-er.md` |
| Events in time | Timeline | `type-timeline.md` |
| Cross-functional handoffs | Swimlane | `type-swimlane.md` |
| Two-axis positioning | Quadrant | `type-quadrant.md` |
| Entities scored across 3–5 criteria | Radar / spider | `type-radar.md` |
| One series across cyclic categories | Polar chart | `type-polar.md` |
| Reinforcing cycle or flywheel | Loop | `type-loop.md` |
| Hierarchy through containment | Nested | `type-nested.md` |
| Parent-child hierarchy | Tree | `type-tree.md` |
| Ownership, reporting, routing, escalation | Org chart | `type-org-chart.md` |
| Stacked abstraction levels | Layer stack | `type-layers.md` |
| Set overlap | Venn | `type-venn.md` |
| Ranked hierarchy or conversion drop-off | Pyramid / funnel | `type-pyramid.md` |
| Category comparison | Bar chart | `type-bar.md` |
| Part-to-whole by area | Treemap | `type-treemap.md` |
| Trends across three or more ordered points | Line chart | `type-line.md` |
| Change between exactly two comparable states | Slopegraph | `type-slopegraph.md` |
| Comparable distributions stacked with overlap | Ridgeline | `type-ridgeline.md` |
| Tasks and phases over time | Gantt | `type-gantt.md` |
| Distribution or correlation | Scatter / bubble | `type-scatter.md` |
| End-to-end data stack on a cluster | High-level | `type-high-level.md` |
| Multi-actor sequential process | Process | `type-process.md` |
| Tiered data storage and quality | Medallion | `type-medallion.md` |
| Role-scoped pipeline flow | Data flow | `type-data-flow.md` |
| Sources → platform core → consumers | DP integration | `type-dp-integration.md` |
| Per-role access permissions | DP security matrix | `type-dp-security-matrix.md` |
| Quantity splitting and merging | Sankey | `type-sankey.md` |
| Causes grouped around one effect | Fishbone | `type-fishbone.md` |
| Value chain against evolution | Wardley map | `type-wardley.md` |
| Work by state, WIP limits, blocked items | Kanban | `type-kanban.md` |
| Experience stages, actions, emotions | User journey | `type-journey.md` |
| Runtime zones, hosts, artifacts, replicas | Deployment | `type-deployment.md` |
| Fan-in, cycles, non-tree dependencies | Dependency graph | `type-dependency.md` |
| Classes, operations, inheritance, composition | UML class | `type-uml-class.md` |
| Narrative backbone sliced into releases | Story map | `type-story-map.md` |
| Physical SQL tables, constraints, indexes, FKs | Database schema | `type-db-schema.md` |

Prefer a paragraph or table when it communicates the same idea. If two types fit, choose the dominant axis; do not combine two layout grammars.