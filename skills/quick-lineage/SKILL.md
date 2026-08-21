---
name: quick-lineage
description: Show an ASCII call graph with per-hop data in- and outflow in chat, fast — no files, no viewer, no gate. Use when the user wants to see the call graph or flow of a function, command, or object under discussion; after changes land, to show what they did to the flow; or before making changes, to show what you are about to do to it.
---

# Quick Lineage

Give the call graph plus the data in- and outflow, straight into chat. Invoke the `show-me` skill (via the Skill tool) — the graph follows its formats. Name the entry and the comparison in one line before running — silently picking is how the wrong graph gets drawn. Use `npx calldiff` (full CLI reference: invoke the `calldiff` skill):

- Current shape: `npx calldiff tree --entry <entry> --maxDepth 2 --locs` (add `<ref>` for a past world)
- What changed: `npx calldiff diff --entry <entry> --maxDepth 2` — git-diff semantics: no refs → HEAD vs worktree
- Planned change: the tree command for the rails, then hand-write the intended `+`/`-` lines — projected, a design's promise, and said so above the block
- "Does this reach X": `npx calldiff reach --entry <entry> --to X`

Entry unknown or not found → `npx calldiff tree --file <path>` expands that file's exports; pick the nearest and say you did. A language calldiff can't parse → trace by hand, marked unverified.

calldiff gives the calls; the source gives the data — Read the files where the tree leaves a hop's data unclear, then annotate every hop with what goes in and what comes out (`args → return`).

Draw the result as an ASCII graph in one fence: boxes for data objects and stores, arrows for calls, each hop carrying its `args → return` and file:line. 

For change or planned views use a ```diff fence instead (`+` green, `-` red). 
Done when every line in the graph is verified, projected, or unverified by name — and one takeaway sentence answers the question that prompted it -> and make sure to color the ASCII for changes.
