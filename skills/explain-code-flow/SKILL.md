---
name: explain-code-flow
description: "Architecture walkthrough: explain how a feature works in the current codebase with brief context, ASCII diagrams, runtime dataflow, and file:line anchors. Use when the user asks for a big-picture explanation, architecture map, or end-to-end feature flow."
---

# Explain Code Flow

Give a little context. Then move from system shape to runtime detail. Use ASD-STE100 Simplified Technical English and Zinsser's four principles of quality writing. Keep source identifiers exact.

## 1. Set the scope

Find the feature, its live entry point, and its final result or effect. Check whether production code assembles and reaches the feature. Show a composition gap when it does not.

Use one or two searches for orientation. Leave the full map to the Explore worker.

Done when the feature boundary and likely source area are clear.

## 2. Map with an Explore worker

Spawn one very thorough Explore worker. Ask it to report, with current `file:line` anchors:

- entry points and composition root
- core types and architectural layers
- protocols and their implementations
- one end-to-end call chain, including data states
- external systems and state changes
- relevant modes, dispatch tables, and catalogs
- approximate file and line counts

Ask for call chains over prose and a report under 1,500 words.

Done when the report reaches from the live entry to the final result and identifies any composition gap.

## 3. Verify and size

Verify each runtime edge, protocol implementation, concrete type, and anchor against the current source. Use LSP call hierarchy when available. Use source search when it is not.

Size the walkthrough by the number of relevant files:

| Files | Target | Required detail |
| --- | --- | --- |
| 1-3 | 200-400 words | overview, abstractions, runtime flow |
| 4-10 | 500-900 words | add relevant conditional sections |
| 11+ | 1,000-1,500 words | add layers and grouped abstractions |

Done when every drawn edge and named type has current source evidence.

## 4. Write one hybrid walkthrough

Use each section for one job:

1. **Context:** State what starts the feature, what it produces, and the scope.
2. **30,000-foot view:** Show the live entry, major components, external systems, final result, and composition gap.
3. **Architecture:** Show layers and dependency direction when the feature crosses layers.
4. **Key abstractions:** List three to seven central types, grouped by layer for a large feature.
5. **Runtime data spine:** Alternate real data values with the functions that transform them. Show loops, fan-in, decisions, I/O, and state changes.
6. **Focused zooms:** Expand only the dense parts of the spine. Keep private subsystem machinery inside its owning zoom.
7. **Conditional reference:** Add ports, modes, routing, catalogs, or reuse evidence only when they help answer the request.
8. **Result:** State the most important fact in one short paragraph.

Use one diagram for each distinct job. The architecture diagram shows where the flow lives. The data spine shows what happens. A zoom explains one dense stage.

When the user asks about private functions or reuse, label non-test source call sites separately from test call sites and add a compact function register.

Done when the architecture and runtime sections form one path without repeating the same call flow.

## 5. Polish and present

Apply `writing-clearly-and-concisely` to the draft. Save it to `ai-docs/explanations/<feature-slug>.md` unless the project defines another location. Show the main diagram in chat and link the complete walkthrough.

Done when the saved file and chat explanation match the current source.

## Diagram rules

- Use terminal-ready ASCII with box-drawing characters.
- Add a clickable `file:line` anchor to each structural claim.
- Use real symbol and type names.
- Keep control paths beside the runtime data spine.
