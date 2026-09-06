# Incremental refresh

Run only for an explicit atlas refresh. Use the same breadth and depth checks
as creation, limited to affected topics and new coverage.

1. Run `python3 <skill>/scripts/atlas.py prepare <atlas>`; optionally pass
   repeated `--target repo=revision`. It captures immutable target commits and
   creates a sibling staging directory. `status: unchanged` means stop: no
   semantic work or rendering is needed.
2. Read the returned changed paths, affected topics, and unmapped paths. Inspect
   source at the captured commits, not a dirty worktree; verify with `git show`.
   Investigate additions for new features, deletions/renames for retired or
   moved facts, and callers/contracts for effects in unchanged files/repos.
3. Update only affected records and figures in the stage. Expand the topic set
   where investigation discovers an impact. Revisit catalogue decisions for
   changed/new subjects. Preserve IDs and geometry where meaning remains the
   same; re-render only changed figures and visually inspect them. Repair source
   ranges moved by edits even when the underlying fact remains true.
4. Account for every changed/unmapped path in `refresh.json` with a compact
   `decisions` list: path, affected topics or why atlas coverage is unchanged.
   Check relevant source bindings, related topics, captions, and glossary.
   Set `reviewed: true` only after this coverage pass.
5. Run `python3 <skill>/scripts/atlas.py publish <stage>`. It validates source
   anchors at the captured pins, rebuilds search and HTML, and replaces the
   published directory only after success. It refuses publication if the
   original atlas changed since preparation. The previous atlas is retained in
   the returned backup directory; report both paths.

On failure, fix the stage and retry. Do not change the published pins by hand.
Retain the old atlas and stage for recovery. If original contents changed,
prepare again and transfer only still-applicable edits. A source revision that
is unavailable requires restoring Git history; never substitute HEAD silently.
