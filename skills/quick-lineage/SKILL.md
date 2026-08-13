---
name: quick-lineage
description: Show a colored call graph in chat, fast — no files, no viewer, no gate. Use when the user wants to see the call graph or flow of a function, command, or object under discussion; after changes land, to show what they did to the flow; or before making changes, to show what you are about to do to it.
---

# Quick Lineage

Give the call stack plus the data in- and outflow, straight into chat. Name the entry and the comparison in one line before running — silently picking is how the wrong graph gets drawn. Use `calldiff` (full CLI reference: invoke the `calldiff` skill):

- Current shape: `calldiff tree --entry <entry> --maxDepth 2` (add `<ref>` for a past world, `--locs` for file:line)
- What changed: `calldiff diff --entry <entry> --maxDepth 2` — git-diff semantics: no refs → HEAD vs worktree
- Planned change: the tree command for the rails, then hand-write the intended `+`/`-` lines — projected, a design's promise, and said so above the block
- "Does this reach X": `calldiff reach --entry <entry> --to X`

Entry unknown or not found → `calldiff tree --file <path>` expands that file's exports; pick the nearest and say you did. A language calldiff can't parse → trace by hand, marked unverified.

calldiff gives the calls; you add the data — annotate each hop with what goes in and what comes out (`args → return`).

Paste inside a ```diff fence (`+` green, `-` red). Done when every line in the graph is verified, projected, or unverified by name — and one takeaway sentence answers the question that prompted it.
