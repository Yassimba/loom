# Exploration brief (one agent per repository)

You document the repository <REPO> so that another agent can later draw many detailed diagrams from your notes WITHOUT reading the code. Read the code thoroughly: package sources, entry points, README, CHANGELOG, Makefile, Dockerfile, CI config, tests overview. Ignore virtualenvs and node_modules.

Write to <WORKDIR>/notes/<repo>.md. Be exhaustive and concrete, always with file:line references. Headed sections:

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

Aim for 300 to 900 lines of dense factual notes. Do not invent. If unsure, say so.

# Cross-repo brief (one agent for the whole product)

Map how the product is composed from all repositories. Write to <WORKDIR>/notes/cross_repo.md. Cover: the package dependency graph with version pins and which repos are libraries versus deployed services; the shared infrastructure inventory as producer, resource, consumer rows; the end-to-end business flow as ordered steps with actor and artifact; the shared data contracts between repos with producer, consumer, format and key fields; runtime topology per environment and how each service is triggered; the release process across repos; gaps you could not determine; 10 to 15 system-level diagram ideas with exact elements.
