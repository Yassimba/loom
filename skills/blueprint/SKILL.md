---
name: blueprint
description: Blueprint a change before building it — Mermaid diagrams of the design, held for approval before any code. Use before implementing non-trivial work — proactively or when the user asks to see the design first — or when another skill needs a design-approval gate.
---

# Blueprint

Show the human a picture of what will be built, get sign-off, then build. The blueprint is the contract for the work that follows.

## 1. Draft

Understand the change from whatever input exists - a user description, spec, ticket, or plan file - then read the code it touches.

**Lineage is the standard axis** — every blueprint carries it, drawn for the one value whose journey the change rewires (the **tracer**):

- The change rewires an existing flow → run **lineage-diff** in prospective mode: BEFORE traced from current source, AFTER projected from this design. The AFTER chain is the design's promise, not a source fact.
- The flow is entirely new (no before world) → a plain lineage of the planned flow per the **lineage** skill.

Take those skills' tracer, tracing, verdict, and diagram grammar.

The future diff carries one sparse details manifest per the lineage skill's `details.md`. Record only delta, open, or inspectable items; `views` maps one semantic entry across lineage, structure, sequence, and other tabs. Every changed hop carries before/after `params` and `returns`; every changed or removed hop lists out-of-flow callers under `impact` (`graphify explain` per symbol); unresolved design carries `open`; added/changed work carries `order`; and an unconsumed planned field carries `flag: "no consumer"`.

The remaining axes are trigger-based - draw every axis whose trigger fires in this change:

| Axis      | Mermaid type      | Draw when the change has…                                                                              |
| --------- | ----------------- | ------------------------------------------------------------------------------------------------------ |
| Flow      | `flowchart`       | control flow the tracer's lineage doesn't carry — a second command, a background job, an error path    |
| Sequence  | `sequenceDiagram` | one temporal slice whose ordering, callback, retry, concurrency, or hand-off is not clear from lineage |
| Structure | `classDiagram`    | a new or reshaped interface — new class, new/changed public methods                                    |
| State     | `stateDiagram-v2` | a lifecycle — states an entity moves through, new/changed transitions                                  |
| Data      | `erDiagram`       | a new or reshaped schema — tables, columns, relationships                                              |

A trigger nominates an axis; it does not require one. Draw it only when it answers a design question lineage leaves unclear. The lineage tab alone can be the whole blueprint; name skipped nominees and why they add nothing.

Scope each diagram to the delta plus its immediate neighbours. Lineage keeps side-by-side worlds; supporting axes use one compact delta view: after-shape items are orange when changed and green when added, removed items remain as red ghosts, and undisclosed context defaults gray. A sequence is a zoom lens over one temporal slice, never a redraw of the lineage. Put each non-unchanged item's verdict once in the shared details manifest; the viewer colors every mapped view.

Write each diagram to its own `.mmd` file — beside the plan artifacts if the work has a docs directory, otherwise a temp location. Syntax lives in the **mermaid-skill** — invoke it when unsure of syntax or when a diagram fails to validate, and take only its Author step and reference files.

Done when:

- the lineage diagram exists — as a diff whenever a before world does, with every touched file in a non-unchanged hop or called out as outside it
- every fired axis adds information the lineage does not already show
- every node, participant, and class names an actual file, module, or actor, or one this change creates — a blueprint of generic boxes approves nothing

## 2. Render

The human approves a picture, not source. Build the page with the **lineage** skill's viewer — one viewer for the whole family; it replaces mermaid-skill's own render step:

```bash
<skills-dir>/lineage/scripts/render.sh -t "<change name>" -o blueprint.html \
  [-d lineage.details.json] lineage.mmd flow.mmd sequence.mmd
```

Name the lineage file `lineage.mmd` and pass it first. Filenames become tab labels; `-d` feeds the shared sparse manifest to every tab, and the viewer applies verdict colors, linked drawers, and keyboard navigation across views. The script validates every diagram in a headless browser; fix parse failures and re-run until it reports OK.

Show the built page through an inline render/preview tool if the harness has one, otherwise `--open`. If the script warns that validation was skipped or nothing rendered (no browser, or offline — the viewer loads Mermaid from a CDN), emit fenced `mermaid` blocks in chat instead, or export each `.mmd` to an image via the **mermaid-skill**.

Alongside the picture, put a lean summary in chat: what gets built, in what order, and what stays untouched — enough to approve without leaving the conversation.

Done when: the page built, validated, and shown, and the summary is in chat.

## 3. Gate

No production code until the blueprint is approved. Ask for a verdict (via AskUserQuestion when available):

- **Approve** — the blueprint is locked; it is the map for the build, and an approved lineage diff is the contract the built code's real lineage must match. Start building.
- **Revise** — take the feedback, redraw, re-render, return to this gate.
- **Rethink** — the approach is wrong; return to Draft from scratch.

If the human asks for a revision you believe is a mistake, explain the trade-off before complying — then draw what they chose.

## 4. Verify (after the build)

The approved lineage tab is the promise; the built code must keep it. Once the build lands, run **lineage-diff** in contract mode with the same tracer — BEFORE is the approved blueprint's AFTER chain and AFTER is built source. For languages calldiff parses (22), ground the built side first: `calldiff diff <ref-at-approval> --entry <entry>` prints the AST-verified call-chain delta the build actually made (`--maxDepth 2` keeps it readable) — drift-verdict against that, not against a fresh hand-trace of the source. The verdicts read as contract outcomes:

- **unchanged** — promise kept
- **changed** — drift: the hop exists but its signature, home, or mechanism differs from the promise — name the difference
- **added** — a surprise the blueprint never showed
- **removed** — promised but never built

Report every non-unchanged hop in one sentence each. Drift the human accepts becomes the new contract — update the blueprint's `.mmd` so the record matches reality.

Done when: the promise-vs-built diff is rendered and every non-unchanged hop is explained or fixed — an all-gray diff is the goal.
