# Changelog

## [Unreleased]

## [0.1.0] - 2026-08-08

- feat(stackdiff): --seq and --er diagram views from call data (`ef3f10a`)
- fix(stackdiff): fit -m graphs to the terminal width (`700b0d7`)
- feat(stackdiff): overhaul -m — left-right layout, tints, docs, links (`404cae6`)
- revert(stackdiff): drop the --img inline-image mode (`709d63f`)
- fix(stackdiff): supersample --img at 3x natural size (`7b0ed31`)
- feat(stackdiff): --img renders the diagram inline in the terminal (`52871e8`)
- style(stackdiff): adopt beautiful-mermaid's box aesthetics in -m (`050dac3`)
- feat(stackdiff): -m boxed-graph terminal view (`8e3f62f`)
- feat(stackdiff): mark call-site changes as a third diff status (`5f78ff5`)
- feat(stackdiff): exclude test files by default (`d48f4c7`)
- feat(stackdiff): prune unchanged limbs by default (`cf1905a`)
- perf(stackdiff): hash-based entry inference, parallel entry diffs (`1e64fca`)
- feat(stackdiff): default depth 3, respect .gitignore (`946d634`)
- feat(stackdiff): git-diff colors, mermaid export, ~11x faster (`c35513a`)
- feat(stackdiff): rich output, pruning, formats, clickable locations (`e83d67f`)
- feat(stackdiff): add Rust call graph support (`cc1117d`)
- refactor!: rename agent setup to Loom (`fb011ce`)
- feat(stackdiff): Rust port of calldiff with a --tree mode (`f0fd265`)
