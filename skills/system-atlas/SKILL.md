---
name: system-atlas
description: Build a one-page HTML atlas of a multi-repository product, hundreds of layered diagrams with plain-language captions and a glossary, from parallel code exploration.
disable-model-invocation: true
---

# System Atlas

One HTML page that teaches a whole product through diagrams: how the repositories connect, then each repository layer by layer, down to algorithms, data models and runtime sequences. Text supports the figures. Built by fan-out: explorers write notes, builders draw from notes, writers rewrite captions, one script assembles.

Requires the `diagram-design` skill and a subagent facility. Its renderer builds every figure.

## Inputs

Collect before step 1. Ask only for what is missing.

- **Repos**: absolute paths of every repository in scope. Add them as working directories.
- **Product name** and one sentence on what it does.
- **Terms**: 20 to 40 domain and tooling words a newcomer would not know. Derive from READMEs if the user does not supply them.
- **Workdir**: the session scratchpad. Layout: `notes/`, `diagrams/<section>/`, `glossary.json`, `atlas.json`.

## Steps

1. **Scaffold.** Invoke the `diagram-design` skill once, record its directory as `<DIAGRAM_SKILL_DIR>`, then read its `references/type-index.md`: that is the full catalogue of diagram types, and every builder prompt below carries it. Create the workdir layout. Copy `briefs/*.md` into the workdir with `<WORKDIR>`, `<PRODUCT>`, `<PROJECT_ROOT>`, and `<DIAGRAM_SKILL_DIR>` filled in. Write `atlas.json` with `title`, `eyebrow`, `intro`. Copy `scripts/assemble.py`, `scripts/nav.css`, and `scripts/nav.js` beside it.
2. **Explore.** One explorer subagent per repo with `briefs/explore.md`, plus one cross-repo subagent, all in one parallel batch. Done when every `notes/<repo>.md` exists and its section 13 lists diagram ideas.
3. **Build diagrams.** One builder agent per section with `briefs/diagram.md`. Section order: system overview first, then repos in data-flow order. Split any repo whose notes exceed about 400 lines into two or three sections (runtime, data models, core algorithm) with fractional `order` values. Ask each builder for 12 to 20 diagrams, top-down: level 1 overview, level 2 subsystems, level 3 zooms. In each builder prompt, list the diagram ideas from the notes, then paste the type catalogue from `type-index.md` and require the builder to consider every type in it and use each one that can teach something about this section, so an ER, a state machine, a swimlane, a sankey or a treemap is never skipped for lack of habit. Builders read `references/type-<name>.md` for each type they use, as `briefs/diagram.md` instructs. Tell builders to write `manifest.json` early and refresh it after every few diagrams, so a killed session leaves usable work. Done when every section has `manifest.json` and every listed html exists.
4. **Assemble and publish a first version.** `python3 assemble.py <WORKDIR>` writes `atlas.html`. Copy it to a stable path the user can open. Publish it through the host's artifact or preview facility when one is available; otherwise give the stable file path now. Readers get value before the polish.
5. **Rewrite captions.** One writer agent per manifest with `briefs/caption.md` and the TERMS list, plus one glossary agent with `briefs/glossary.md`, all in one message. Stance is Improve: reference text, no author voice. The brief already carries the three-paragraph shape (lead, how it works, look here), so one pass is enough. Done when every manifest's intro and captions have that shape and `glossary.json` exists.
6. **Reassemble and republish** to the same artifact URL. Replace em dashes in section titles with colons. Diagram labels inside SVGs stay untouched. Take one dark-mode screenshot and one light one before publishing; maximize one diagram and exercise its zoom controls during this check.

## Recovery

Agents may stop when the session ends, but retained transcripts survive. Check what is on disk (`diagrams/*/`, then compare HTML files with each manifest). Resume each incomplete run by its retained ID with: "write manifest.json first for what exists, then continue". Never relaunch from scratch while a transcript exists. Near the spend limit, stop expendable builders through the host's subagent controls, let the rest finish, then assemble what exists.

## What the page does

`assemble.py` is the single source of the page's behaviour. Every feature below is generated from the manifests, so a rebuild never loses it:

- **One colour per section** (sidebar dot, section rule, overview-figure border, legend under the intro). The legend label is the section title up to the first colon, so title sections as `repo: what it does`.
- **Collapsed detail.** Consecutive level-3 figures fold under their level-2 parent behind "Show N detail figures". Figures whose title matches the build-and-deploy words (ci, cicd, ci/cd, release, docker, deploy, deployment, makefile, test structure, integration tests, versioning) fold into one "Build, deploy and tests" group per section, except in the first section, which keeps the release story open. Builders title such figures with those words so the fold catches them.
- **Hover terms.** The first mention of a glossary term in each caption gets a dotted underline and an instant custom tooltip (a `data-tip` attribute read by `nav.js`, never the browser `title`, which delays about a second and cannot be styled). Terms are focusable, so keyboard users get the same tooltip. Aliases split on "and", commas and slashes, so a glossary term like "Medium voltage and low voltage" matches both halves.
- **Sidebar**: collapsible sections, scroll tracking, "Now viewing" with "Figure n of N · Section s of S", a search box over titles and captions, and a mobile "Browse contents" toggle. Markup and script live in `scripts/nav.css` and `scripts/nav.js`.
- **Diagram viewing**: every figure has one quiet, right-aligned icon row in this order: zoom out, current percentage, zoom in, reset, then Maximize after a divider. Maximize opens the complete figure across the viewport and keeps the controls available; its icon becomes Exit full screen. `Esc` also exits. Zoom levels are 100%, 125%, 150% and 200%, and horizontal scrolling keeps enlarged diagrams reachable. Hide Maximize and its divider when the Fullscreen API is unavailable.
- **Theme toggle**: system, light, dark, remembered in the browser. Dark is warm graphite. Diagrams are recoloured, not filtered: the builder collects every fill and stroke value in the SVGs and emits an exact dark rule for each one it knows, from the `DARK` map at the top of the script. That map is keyed on the diagram-design profile palette (ink 2d3142, muted 4f5d75, paper f5f5f5, accent eb6c36, soft 7a8399, rule bfc0c0, link 2e5aa8). A project with another profile updates the map first, or its diagrams stay light in dark mode.
- **SVG hygiene.** Every id inside a figure is prefixed with the figure id, so 204 figures do not share one `#arrow` marker, and `color-mix()` tints become `rgba()` so Safari paints them.

## When someone edits the HTML directly

The HTML is the deliverable, the manifests and `assemble.py` are the source. If another agent or person improves the HTML in place, diff it against the last build, port CSS and script changes into `nav.css`, `nav.js` or `assemble.py`, and port SVG or caption changes into the builder or the manifests. Only then rebuild. Republishing their file as-is is fine for one round, but the next rebuild silently drops their work unless it was ported.

## Manifest contract

`diagrams/<section>/manifest.json`:

```json
{"section": "id", "title": "Repo: what it does", "order": 3, "intro": "...",
 "diagrams": [{"file": "01-slug.html", "title": "...", "type": "sequence", "level": 1, "caption": "..."}]}
```

`assemble.py` extracts the `<svg>` from each html, orders sections by `order`, nests figures by `level` in the sidebar, appends the glossary last. JSON stays the editable source; the html is the deliverable.
