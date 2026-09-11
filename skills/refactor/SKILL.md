---
name: refactor
description: Strict refactor review with visual before/after proposals and verified implementation.
disable-model-invocation: true
---

# Refactor

## 1. Inspect

Review the user's scope. Otherwise review the current uncommited changes, including dirty and untracked source. Exclude generated, vendored, and build output.

Read project instructions, the changed implementation, relevant callers, and test, and for broad or multi-subsystem changes, use subagents

Send out subagent per topic:

- general simplification: [NUCLEAR](topics/nuclear.md)
- dependencies or custom machinery: [OSS](topics/oss.md)
- recurring abstractions or indirection: [patterns](topics/patterns.md)
- changed or repetitive tests: [tests](topics/tests.md)
- casts, optional state, or weak invariants: [types](topics/types.md)

Present the proposed refactor(s) directly in chat. Use fenced `mermaid` blocks so the built-in renderer displays them.There is support for flowchart, ER, class diagrams, gitgraph, mindmap, pie, sequence , state and timeline.

Use all appropriate diagram types to present the change and add before and after codesnippet. Mark before and after explicitely.

In the diagram use these classes:

- `:::red` removed
- `:::green` added
- `:::orange` changed

To mark changes, DON'T add your own color, keep background transparent, these classes already get colored automatically by the rendered.

## 4. Apply

Ask for approval to implement the proposals.

Task / scope:
$ARGUMENTS
