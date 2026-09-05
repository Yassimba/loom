# Exploration brief (one agent per repository)

You document the repository <REPO> so builders can draw detailed diagrams from its topic records without repeating discovery. Read committed source thoroughly: packages, entry points, README, CHANGELOG, Makefile, Dockerfile, CI config, and tests overview. Ignore virtualenvs and node_modules.

Write persistent `<WORKDIR>/topics/<id>.json` records using `references/records.md`, supplied by the parent. Inspect pinned committed source. Be exhaustive and concrete, with exact ranges and quoted anchor lines. Split records by coherent subsystem/feature; their facts collectively cover:

1. Purpose in one paragraph. Position in the landscape: which sibling repos it imports, who consumes its output.
2. Module layout, one line per module, plus an import graph between modules.
3. Entry points, CLI arguments, environment variables, run modes, how it is triggered in production.
4. End-to-end runtime flow as ordered steps: function names, file:line, data in and out per step, every external read or write (storage keys, tables, queues, HTTP endpoints). Include branches and decisions.
5. All data models: classes, dataclasses, pydantic models, enums, DataFrame or table schemas, with fields, types and relationships. Enough for ER and UML class diagrams.
6. The core algorithm or computation, step by step, with the formulas or rules it applies.
7. State machines and lifecycles (job status, record status, retries).
8. Actors and external systems and the order they interact in, enough for sequence and swimlane diagrams.
9. Configuration and parameters, with the fields actually used.
10. Error handling, logging, tracing, retries.
11. Build, CI, deployment, versioning.
12. Testing structure.
13. 10 to 20 diagram ideas that best explain this repo, each with a diagram type and the exact elements it should contain.
14. Quirks, dead code, stale docs and anything you could not verify, listed separately from facts.

Each fact has an ID and source IDs; topics name questions, terms, file dependencies, and dependent topic IDs. Keep unresolved claims separate. Propose distinct figure questions for the builder. These records replace exploration notes and later evidence packets; retain their factual depth.

# Cross-repo brief (one agent for the whole product)

Map how the product is composed from all repositories. Write cross-repository topic records under `<WORKDIR>/topics/`. Cover: the package dependency graph with version pins and which repos are libraries versus deployed services; the shared infrastructure inventory as producer, resource, consumer rows; the end-to-end business flow as ordered steps with actor and artifact; the shared data contracts between repos with producer, consumer, format and key fields; runtime topology per environment and how each service is triggered; the release process across repos; gaps you could not determine; 10 to 15 system-level diagram ideas with exact elements.
