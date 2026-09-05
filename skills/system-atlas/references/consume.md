# Atlas-first consumers

Shared procedure for Blueprint, explain-code-flow, and code-review. Read this
reference directly; building or refreshing the shared atlas is a separate user
request. Resolve `<atlas-skill>` from the installed system-atlas directory.

## Retrieve

1. Use the user's atlas path, otherwise `<repo>/ai-docs/atlas`. If absent, use
   targeted source inspection; load the visual branch only when needed. Treat
   evidence freshness and reusable geometry separately: an older atlas without
   topic records/pins is unverified evidence, but its diagrams remain candidates
   for focused views after checking the relevant source.
2. Run `python3 <atlas-skill>/scripts/atlas.py search <atlas> '<question or symbols>'`.
   Load matching IDs with `show`; page only when more facts are needed. Load
   selected figure handles with `figure <atlas> <figure-id> --offset 0 --limit 10`. Avoid
   reading HTML, whole exploration notes, and coordinate JSON into context.
   If topic lookup fails, search titles/questions in `diagrams/*/manifest.json`.
   For selected figures, extract only IDs, labels, edge endpoints and bindings
   from their JSON with a local script. Read the selected SVG only when editing a view.
   Missing topic records do not require an atlas rebuild or a redraw.
3. Run `affected <atlas>` once for changes from pinned commits to current HEADs
   (or use repeated `--target repo=revision`). If pins are absent, verify only
   the selected path against current source and state that the baseline is
   unverified; keep unaffected layout. Also inspect relevant staged,
   unstaged, and untracked source when the task targets the working tree.
   Record the task target separately from atlas pins and review comparison base.
4. Inspect changed source and affected callers/contracts using CodeGraph when
   indexed, otherwise targeted reads. Verify relevant bindings against the
   task's revision. Inspect both sides of review diffs. Atlas context is not a
   substitute for reviewing changed code or checking uncertain claims.

Done when selected facts are identified, relevant drift is understood, and new
facts have source references in the consumer's document. Keep shared atlas
files unchanged. Use references to retained facts instead of rewriting them.

## When figures help

For an explanation, proposal, or finding that needs a figure, read
[overlays.md](overlays.md). It covers direct atlas overlays, historical bindings,
and Mermaid fallback. A text-only review stops after retrieval.
