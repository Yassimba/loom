---
name: lineage-diff
description: Render a change as a before/after lineage diff, diff-colored per hop. Use when the user wants a before/after view of what a change did to a flow, or when another skill needs a session's work explained as a flow diff.
---

# Lineage Diff

One change, one **tracer**, two **worlds**. The tracer is a single value or symbol whose journey the change rewired; the worlds are the codebase before and after. Trace the tracer's lineage in each world, give every hop a diff **verdict**, and render the two chains side by side.

Hop grammar (data/call alternation), tracing discipline, and wtf lines come from the **lineage** skill — load it and read §1–3 before tracing, §4 again at render; this skill adds only the second world and the coloring.

## 1. Pick the tracer

Identify the change (session diff, `git status`/`git diff`, or the range the user names), then choose the one value whose lineage crosses the most modified code. A change with two disjoint flows gets two diffs, not one crowded picture.

Done when: the tracer is named and every substantially modified file either sits on its path or is explicitly called out as outside this diff.

## 2. Trace both worlds

Choose the mode from what the worlds are:

- **actual diff** — BEFORE is the base checkout; AFTER is the committed tip or working tree. Trace BEFORE in `git worktree add --detach <scratch>/before <base>`, then remove it. Verify every hop against its world's source.
- **prospective diff** — BEFORE is current source; AFTER is a proposed design. Verify BEFORE; mark AFTER signatures as intentional projections, carry unresolved choices under `open`, and never describe them as source facts.
- **contract diff** — BEFORE is an approved blueprint's AFTER chain; AFTER is built source. The blueprint is the promise, so every difference is implementation drift.

For worlds that exist as real source (both sides of an actual diff, the BEFORE of a prospective diff, the AFTER of a contract diff) in a language stackdiff parses (TS/TSX, Python, Go), seed the call skeleton with `stackdiff <base> <tip> -e <entry>` — an AST-verified diff of who-calls-whom between two git refs, working tree included when `<tip>` is omitted (`stackdiff --tree <ref> -e <entry>` prints one world's chain, `--max-depth 2` keeps either readable). Its `+`/`-` lines pre-answer added/removed for call hops. It knows nothing of data hops, wtf lines, or design projections — those stay yours, and a projected world is never stackdiff's to describe.

Both chains share entry and terminal. If either changed, anchor at the nearest unchanged ancestor and descendant. A graphify graph describes only the world whose files it indexed; check paths and freshness rather than trusting its stamped commit.

Done when: two chains exist, alternation holds, and every hop states whether it was source-verified or projected from the approved design.

## 3. Verdict every hop

Exactly one verdict per hop, in both chains. Verdicts are scoped to the named change, not every difference currently present in the working tree:

- **unchanged** — same symbol, same home, same mechanism; unrelated working-tree dirt also stays unchanged
- **changed** — survives in both worlds but its signature, home module, mechanism, or position in the chain moved (a rename alone is changed)
- **removed** — exists only in BEFORE
- **added** — exists only in AFTER

Done when: every hop carries a verdict and every modified file from step 1 appears in at least one non-unchanged hop — a diff that colors nothing has the wrong tracer.

## 4. Render

Two files, in the same home as lineage §4's `.mmd`:

- `lineage-diff-<slug>.mmd` — the full hop chain, one `flowchart LR`:
  - two subgraphs, `BEFORE` (direction TB) and `AFTER` (direction TB), pinned side by side with an invisible edge `BEFORE ~~~ AFTER` (without it the worlds float apart and stack)
  - the legend lives in the viewer's header — the `.mmd` holds only the two worlds
  - node shapes follow lineage grammar; verdict color comes from the details manifest, so the `.mmd` carries no repeated palette block
  - node IDs `B_<module>__<hop>` / `A_<module>__<hop>` — the shared suffix pairs a hop with its counterpart in the other world, so counterparts must share it exactly
  - lean labels (signature + wtf line); depth lives in the details file
- `lineage-diff-<slug>.details.json` — a sparse manifest authored per the lineage skill's [details.md](../lineage/details.md). Include every non-unchanged hop and only unchanged hops worth inspecting. Gather facts with `graphify explain "<symbol>"`, targeted reads, or the BEFORE worktree.

Removed hops appear only in BEFORE, added only in AFTER; a cross-cutting addition (a protocol, a shared helper) is one added node with dotted edges into its consumers. Each changed/added node's wtf line says what *about it* changed, in plain words. Subgraph titles carry a five-word summary of each world's character.

The viewer owns the fixed gray/orange/red/green palette. Undisclosed hops default gray; manifest verdicts color the delta.

Build the viewer — the lineage skill's script owns the HTML:

```bash
<skills-dir>/lineage/scripts/render.sh -t "<change name>" -r "<base>..<tip>" -d lineage-diff-<slug>.details.json lineage-diff-<slug>.mmd
```

`-r` stamps the diff range into the header so a saved page stays anchored to what it describes; omit it for an uncommitted change and the script stamps the current commit plus a working-tree marker itself.

The page provides platform-native keyboard navigation, graph-aware branch switching, and BEFORE/AFTER crossing; `#<node id>` deep-links a hop. Two disjoint flows become separate tabs. Do not open it unless the user asks. Browser validation follows lineage §4: it is optional, explicit, and may use only a dedicated `LINEAGE_HEADLESS_BROWSER`. Never discover, launch, or pass the path of the user's desktop Chrome/Chromium/Edge installation, and never fall back to Puppeteer or `mmdc` automatically.

Done when: the HTML exists and the response states whether dedicated browser validation ran. An unvalidated artifact is acceptable by default.
