---
name: quick-lineage
description: Show a colored call graph in chat, fast — no files, no viewer, no gate. Use when the user wants to see the call graph or flow of a function, command, or object under discussion; after changes land, to show what they did to the flow; or before making changes, to show what you are about to do to it.
---

# Quick Lineage

One colored call graph in chat, grounded in `stackdiff` (AST-verified for TS/TSX, Python, Go) — the fast sibling of **lineage** and **blueprint**, for when the picture matters more than the ceremony.

## 1. Read the branch from the conversation

Infer both the **entry** (the function, command, or object's method the conversation centers on — with several candidates, the one whose call tree covers the most of what was discussed) and the **branch**:

- **Discussed** — the user is asking about something as it is → one world, current source.
- **Landed** — changes exist (session edits, working tree, a named ref) → diff the worlds.
- **Planned** — the change is still intent → real BEFORE rails plus a projected `±` overlay.

State the choice in one line ("quick lineage of `run`, landed changes vs HEAD") before running anything. Done when: the entry and branch are named in chat — silently picking is how the wrong graph gets drawn.

## 2. Run stackdiff

Start at `--max-depth 2` and deepen only the limb the question lives in — full depth buries the point.

| Branch | Command |
| --- | --- |
| Discussed | `stackdiff --tree -e <entry> --max-depth 2` (add `<ref>` for a past world) |
| Landed | `stackdiff <base> -e <entry> --max-depth 2` — working tree included; add `<tip>` for ref-to-ref |
| Planned | the Discussed command for BEFORE rails, then hand-write `+`/`-` lines for the intended calls |

Entry not found → `stackdiff --tree` lists the exported entrypoints; pick the nearest and say you did. Language outside TS/TSX/Python/Go → say stackdiff cannot parse it and trace by hand, marked as unverified.

## 3. Show it colored

Paste the output in a fenced ` ```diff ` block — the chat renders `+` green and `-` red; keep stackdiff's two-space status column so rails align. Trim rails far from the delta to one `  …` line. For **Planned**, projected lines are the design's promise, not source facts — say so above the block.

Under the block, one takeaway sentence: what the graph shows about the question that prompted it.

Done when: the graph is in chat with every `+`/`-` line either machine-verified by stackdiff or explicitly marked projected, and the takeaway names what changed or will change.
