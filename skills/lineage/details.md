# details.json — the drawer's contract

Entries are sparse: add one for every changed, added, removed, open, or inspectable item; undisclosed items remain navigable and default to unchanged. On a diff page, clicking a node shows its BEFORE and AFTER entries together. On a plain lineage page keys are bare item ids and `verdict` is omitted.

One semantic entry may drive several diagrams through `views`, avoiding repeated verdicts and explanations:

```json
{"A_resolve": {
  "label": "ProjectConfig.from_declaration", "verdict": "changed",
  "views": {
    "lineage": "A_resolve",
    "structure": "ProjectConfig",
    "sequence": "message_2"
  }
}}
```

Flowchart ids are the authored node ids, class ids are class names, and sequence messages are `message_0`, `message_1`, … in visible order. `views` values may be a string or an array when one semantic item appears more than once in a view. Every mapped item opens the same drawer entry and receives the same verdict color.

```json
{"A_parsing__manifest": {
  "label": "Manifest", "verdict": "added", "kind": "data",
  "signature": "class Manifest(BaseModel)",
  "doc": "A validated manifest.",
  "wtf": "the typed payload the parser now hands over",
  "fields": [{"name": "entries", "type": "list[Entry]", "verdict": "added"}],
  "notes": ["replaces the untyped dict that leaked parser internals"],
  "source": {"path": "parsing/models.py:5", "href": "<editor template>/parsing/models.py:5"}
}}
```

All keys but `label` are optional.

- `verdict` — the item's diff verdict; the viewer applies the shared palette on flow, class, and sequence tabs
- `kind` — function / method / classmethod / staticmethod / data
- `class` — a method's owning class
- `signature` — exactly as written in source
- `params` (`[{"name", "type"}]`) and `returns` — render as a type table
- `raises` — the exceptions the hop can raise, one string exactly as annotated (e.g. `"ManifestError"`); diffed old→new like `returns`, because an error-behavior change is as breaking as a type change
- `doc` — the docstring's first line
- `wtf` — the hop's wtf line
- `fields` — a data object's fields, each with its own verdict: the payload's shape diff
- `notes` — what changed and why it matters, one point per string
- `source` — `path` is repo-relative `file:line`; `href` follows the repo's editor link template, the `### Editor` entry in the `## Agent skills` block of `CLAUDE.md`/`AGENTS.md` (written by **setup-project**). No entry ⇒ `vscode://file/{path}:{line}` with `{path}` absolute; editor "None" ⇒ omit `href` and keep `path`.
- `views` — item ids carrying this same semantic entry across diagram tabs

Scope keys — how a diff explains its own footprint (a blueprint's future diff carries all four):

- `impact` — `[{"caller", "where", "note", "href"}]`: callers *outside* the traced flow that must adapt to this hop's change (gather via `graphify explain`); the drawer renders it as the hop's blast radius. `where` is `file:line`; `href` is optional and follows the same editor link template as `source.href`, turning the location into a deep link
- `open` — an unresolved design question, one sentence; the node renders dashed and the drawer shows the question — the approval conversation starts at the dashed nodes
- `order` — build sequence number (integer); the node wears it as a badge
- a field's `flag` — short warning on one data field, e.g. `"no consumer"` for a planned field no hop reads

When BEFORE and AFTER entries both carry `params`/`returns`/`raises`, the drawer diffs them automatically — added, removed, and retyped parameters are chipped per row, so a changed hop's entry needs no prose about *what* changed, only *why*.
