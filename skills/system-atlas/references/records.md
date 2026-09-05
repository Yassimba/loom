# Atlas records

The atlas directory is durable source, not a scratchpad. Keep `atlas.json`,
`topics/*.json`, `diagrams/<section>/manifest.json`, semantic figure JSON sidecars,
editable diagram HTML, and `glossary.json`. `search.json` and `atlas.html` are generated.
Use the installed scripts; do not copy them into each atlas.

## Repository baseline

Add `repositories` to the existing `atlas.json` title/eyebrow/intro:

```json
{"repositories": [{"id": "api", "path": "../..", "commit": "FULL_COMMIT_ID"}]}
```

Paths resolve from the atlas directory. For multiple repositories use distinct
IDs and paths (absolute paths are allowed for machine-local repositories).
An optional `github` HTTPS repository URL enables pinned source permalinks.
Otherwise the HTML links to Cursor only when the recorded range still matches
current disk contents; moved/stale ranges remain labeled historical references.
Explore committed source at the captured commits, even if the worktree is dirty.
An old atlas without pins stays viewable; verify its facts before assigning a
baseline. Never label an unverified historical atlas with current HEAD.

## Topic contract

One `topics/<id>.json` per coherent feature, subsystem, model, or algorithm:

```json
{
  "id": "api.session",
  "section": "api",
  "title": "Session creation",
  "summary": "Requests create one durable session.",
  "questions": ["Where is a session persisted?"],
  "terms": ["session", "deduplication"],
  "facts": [{"id": "write", "text": "createSession stores the session.", "sources": ["store"]}],
  "sources": [{"id": "store", "repo": "api", "path": "src/session.ts",
    "symbol": "createSession", "start": 40, "end": 66,
    "anchor": "export function createSession(input: SessionInput) {"}],
  "dependencies": [{"repo": "api", "path": "config/session.json"}],
  "dependsOn": ["api.identity"],
  "figures": [{"id": "session-write", "json": "diagrams/api/01-session.json",
    "question": "Where is a session persisted?"}]
}
```

Every fact cites local source IDs; source anchors quote an exact line within
the recorded range at that repository's commit. Keep unresolved claims in an
optional `unknowns` list. `dependencies` records exact additional file paths;
`dependsOn` records topic dependencies, including across repositories. Cycles
are allowed. New/unmapped files are always returned for investigation.
Keep facts atomic and cite only the ranges that support them. `show` pages
related topics, file dependencies, unknowns, and figure summaries alongside
facts; follow `nextOffset` when more context is needed.

IDs survive reordering and refresh. Every diagram element that can be selected
for an overlay has a unique `id`, including edges, zones, and primitives.
Each manifest diagram row retains `file`, `json`, `question`, `title`, `type`,
`level`, `caption`, and adds its stable `id` and owning `repo` ID. Bind existing
code with a `code` array in the semantic sidecar and matching `data-element-id`,
`data-repo`, and comma-separated `data-code` attributes on the SVG element.
Sidecars contain IDs, labels, edge endpoints and bindings, not geometry;
HTML is the visual source. See `briefs/diagram.md` for the authoring contract. For cross-repository figures, use
separate elements and include their repository in the topic source records.

The manifest also records `coverage`: a short list of applicable subjects and
their topic/figure IDs; `typeDecisions`: one compact decision per catalogue
type; and `depthCheck`: remaining generic boxes or why coverage is complete.
If a substantial section has fewer than 12 figures, record `quotaReason` after
checking for missing subjects and zooms. These are authoring decisions, not
proof that the documented code is correct.

`typeDecisions` rows use `{"type": "Architecture", "subject": "service boundaries"}`
or `{"type": "Sankey", "reason": "No supported flow quantities"}`. Use the
catalogue's exact type names. The validator checks that every row was considered,
coverage/depth fields exist, and a sub-12 section has a quota explanation.

## Commands

Run `python3 <system-atlas>/scripts/atlas.py --help`. `index` validates committed
anchors and generates human/agent search data. `search` returns five matches
by default. `show` pages facts and their sources with `--offset` / `--limit`;
`figure` pages a selected figure's semantic handles without layout. `freeze` writes selected topic records for a
consumer's reproducible snapshot. Keep topic summaries short; split oversized
topics at a real subsystem boundary.

`validate` checks source anchors at pinned commits, references, and element
identities. It cannot prove semantic coverage: that is the explorer's job.
