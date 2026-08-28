---
name: blueprint
description: Blueprint a change before building it — three ASCII views of the design, held for approval before any code. Use before implementing non-trivial work — proactively or when the user asks to see the design first — or when another skill needs a design-approval gate.
---

# Blueprint

Show the human three ASCII pictures of what will be built, get sign-off, then build. The pictures are the contract.

Invoke the `show-me` skill (via the Skill tool) — formats, diffs, and captions follow it. No files, no viewer, no Mermaid unless the user asks. Chat only.

## 1. Draft

Understand the change from whatever exists — a description, spec, ticket, or plan — then read the code it touches. Name the **tracer**: the one value or symbol whose journey the change rewires. Name the entry. Silent pick = wrong picture.

## 2. Show

Three views, in this order. Each sits next to one sentence. Real names only — generic boxes approve nothing.

### Where it sits

One layer up. Modules and callers around the change, in the project's domain words. A shallow file or component tree is enough. Use a `diff` fence when the layout moves.

```text
cli/
├── commands/     # parses the user action
└── sessions/     # owns session state   ← change lives here
      transport/  # sends API requests
```

### The flow

ASCII graph of the tracer, entry to exit. Boxes for data, arrows for calls, `args → return` and file:line on each hop.

Existing flow → `diff` fence: current rails, intended AFTER as `+`/`-`, marked **projected**. New flow → one `text` block, marked projected.

```diff
 on(save)
   read content
-  write content
+  if content is unchanged
+    return cached result
+  write new content
   return result
```

### The callgraph

`npx calldiff tree --entry <entry> --maxDepth 2 --locs` for the rails, then hand-write the intended `+`/`-` (full CLI: invoke `calldiff`). Entry unknown → `--file <path>`, say you picked. Language calldiff can't parse → hand-trace, marked **unverified**.

```diff
 submit_form
   create_session
     persist_prompt
+    expand_skill_mention
     launch_agent
```

Skip a view only when it adds nothing the others already show, and say why. Extra show-me views (sequence, state, schema) only when these three leave a design question unanswered.

Done when: the three views (or the named skips) are in chat, every name is a real file/symbol/actor, and one takeaway says what gets built, in what order, and what stays untouched.

## 3. Gate

No production code until approved. Ask (AskUserQuestion when available):

- **Approve** — the pictures lock; they are the map. Start building.
- **Revise** — take the feedback, redraw, return here.
- **Rethink** — approach is wrong; return to Draft.

If they ask for a revision you believe is a mistake, say the trade-off, then draw what they chose.

## 4. Verify (after the build)

Same three views against built source. Ground the callgraph with `npx calldiff diff <ref-at-approval> --entry <entry> --maxDepth 2`. Report every `+`/`-` in one sentence. Accepted drift updates the pictures so the record matches reality.

Done when: promise-vs-built is in chat and every surprise is explained or fixed.
