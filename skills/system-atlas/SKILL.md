---
name: system-atlas
description: Build a detailed, searchable, code-bound product atlas; refresh it incrementally from pinned Git commits.
disable-model-invocation: true
---

# System Atlas

One HTML page teaches the product through layered diagrams, algorithms, models, and runtime sequences. Persistent topic records make the same knowledge searchable by agents. Explorers establish facts once; builders draw and caption them; scripts index and assemble.

Creation uses `diagram-design` and parallel explorers/builders when available. Without subagents, follow the same section boundaries sequentially.

For **refresh**, read [references/refresh.md](references/refresh.md) and [references/records.md](references/records.md), then follow that branch instead of rebuilding. Consumers load [references/consume.md](references/consume.md) directly; they do not run atlas creation.

## Inputs

Collect before step 1. Ask only for what is missing.

- **Repos**: absolute paths of every repository in scope. Add them as working directories.
- **Product name** and one sentence on what it does.
- **Terms**: 20 to 40 domain and tooling words a newcomer would not know. Derive from READMEs if the user does not supply them.
- **Atlas directory**: the user's path or `ai-docs/atlas/` in the main repo. Durable layout: `topics/`, `diagrams/<section>/`, `glossary.json`, `atlas.json`.
- **Baseline**: capture one full commit ID per repository. Explore that committed source; working-tree changes belong to consumer overlays.

## Steps

1. **Scaffold.** Read [references/records.md](references/records.md). Invoke `diagram-design` once and record its directory. Read its full visual-type guide in `SKILL.md`. Create the atlas directory and `atlas.json` with title, introduction, and repository pins. Use installed scripts directly. Done when repository identities and baseline commits are explicit.
2. **Explore.** One explorer per repo with `briefs/explore.md`, plus one cross-repo explorer. Their output is persistent topic records, not a second evidence packet. Done when topics cover real entry points, models, algorithms, state, handoffs, failures, deployment, and contracts, with exact committed source anchors and explicit unknowns.
3. **Select and build.** Give each section builder `briefs/diagram.md`, relevant topics, and the full type catalogue. Consider every type and record compact `typeDecisions`. Target 12–20 figures per substantial section; fewer triggers a coverage check and `quotaReason`, more is allowed. Require meaningful zooms beneath generic boxes. Record subject-to-figure `coverage` and a final `depthCheck`. Read detailed type references only for selected types. Write captions once in the lead → mechanism → caveat/source shape. Write manifests early and update them during construction. Done when every applicable subject has coverage, every retained figure answers a distinct question, its JSON has stable element IDs and source bindings, and rendering passes inspection.
4. **Glossary and coverage challenge.** Use `briefs/glossary.md` with the topic records. Check what behavior remains hidden and why less familiar applicable types were rejected. If nearly everything is architecture, sequence, or flowchart, revisit the catalogue decisions. Quantitative figures require supported quantities. Done when gaps are filled or explicitly unresolved, not hidden by diagram count.
5. **Validate and publish.** Run `python3 <skill>/scripts/atlas.py index <atlas>` then `python3 <skill>/scripts/assemble.py <atlas>`. Inspect light/dark themes, search by symbol and domain term, and maximize/zoom one figure. Publish the stable `atlas.html` path. Done when humans can navigate its details and agents can retrieve selected topics without reading HTML or geometry.

## Recovery

Agents may stop when the session ends, but retained transcripts survive. Check what is on disk (`diagrams/*/`, then compare HTML files with each manifest). Resume each incomplete run by its retained ID with: "write manifest.json first for what exists, then continue". Never relaunch from scratch while a transcript exists. Near the spend limit, stop expendable builders through the host's subagent controls, let the rest finish, then assemble what exists.

## What the page does

`assemble.py` is the single source of the page's behaviour. Every feature below is generated from the manifests, so a rebuild never loses it:

- **One colour per section** (sidebar dot, section rule, overview-figure border, legend under the intro). The legend label is the section title up to the first colon, so title sections as `repo: what it does`.
- **Collapsed detail.** Consecutive level-3 figures fold under their level-2 parent behind "Show N detail figures". Figures whose title matches the build-and-deploy words (ci, cicd, ci/cd, release, docker, deploy, deployment, makefile, test structure, integration tests, versioning) fold into one "Build, deploy and tests" group per section, except in the first section, which keeps the release story open. Builders title such figures with those words so the fold catches them.
- **Hover terms.** The first mention of a glossary term in each caption gets a dotted underline and an instant custom tooltip (a `data-tip` attribute read by `nav.js`, never the browser `title`, which delays about a second and cannot be styled). Terms are focusable, so keyboard users get the same tooltip. Aliases split on "and", commas and slashes, so a glossary term like "Medium voltage and low voltage" matches both halves.
- **Sidebar**: collapsible sections, scroll tracking, "Now viewing" with "Figure n of N · Section s of S", a search box over titles, captions, and indexed topic facts/symbols, and a mobile "Browse contents" toggle. Markup and script live in `scripts/nav.css` and `scripts/nav.js`.
- **Diagram viewing**: every figure has one quiet, right-aligned icon row in this order: zoom out, current percentage, zoom in, reset, then Maximize after a divider. Maximize opens the complete figure across the viewport and keeps the controls available; its icon becomes Exit full screen. `Esc` also exits. Zoom levels are 100%, 125%, 150% and 200%, and horizontal scrolling keeps enlarged diagrams reachable. Hide Maximize and its divider when the Fullscreen API is unavailable.
- **Theme toggle**: system, light, dark, remembered in the browser. Dark is warm graphite. Diagrams are recoloured, not filtered: the builder collects every fill and stroke value in the SVGs and emits an exact dark rule for each one it knows, from the `DARK` map at the top of the script. That map is keyed on the diagram-design profile palette (ink 2d3142, muted 4f5d75, paper f5f5f5, accent eb6c36, soft 7a8399, rule bfc0c0, link 2e5aa8). A project with another profile updates the map first, or its diagrams stay light in dark mode.
- **SVG hygiene.** Every id inside a figure is prefixed with the figure id, so 204 figures do not share one `#arrow` marker, and `color-mix()` tints become `rgba()` so Safari paints them.

## When someone edits the HTML directly

The HTML is the deliverable, the manifests and `assemble.py` are the source. If another agent or person improves the HTML in place, diff it against the last build, port CSS and script changes into `nav.css`, `nav.js` or `assemble.py`, and port SVG changes into the individual diagram HTML and caption changes into the manifests. Only then rebuild. Republishing their file as-is is fine for one round, but the next rebuild silently drops their work unless it was ported.

## Manifest contract

`diagrams/<section>/manifest.json`:

```json
{"section": "id", "title": "Repo: what it does", "order": 3, "intro": "...",
 "diagrams": [{"file": "01-slug.html", "json": "01-slug.json", "title": "...",
   "type": "sequence", "level": 1, "question": "Where does a sync write?",
   "caption": "..."}]}
```

`assemble.py` extracts the `<svg>` from each html, orders sections by `order`, nests figures by `level` in the sidebar, appends the glossary last. Individual diagram HTML is the editable visual source; JSON is a semantic inventory of IDs, labels, edges and source bindings, without geometry. `question` is how Blueprint picks a figure to overlay instead of redrawing.

## Reuse existing figures

For later change plans, follow [references/overlays.md](references/overlays.md). This preference does not change atlas creation.
