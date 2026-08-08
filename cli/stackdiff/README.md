# stackdiff

`git diff` for who-calls-whom. Point it at a repo and it shows how call
graphs changed between two git states — extracted statically from the AST
(TypeScript/TSX, Python, Go, Rust), rendered in your terminal, colored like
a diff: green added, red removed, amber for same-call-different-arguments.

```
stackdiff main → working tree
61 entries changed · +1,204 −890 !77 · biggest: drain_run (+210 −8 !3)

  execute_and_render(session, target, request) → ExitStatus  check.py:185
  │  “Resolve, drive the run through the observers, and render the report.”
- ├─ drive_run(session, target, request) → DrainOutcome
+ ├─ run_and_report(session, target, request) → tuple[RunReport, str]
+ │  │  “Drive the run: resolve once, stream through the observers, finalize.”
+ │  ├─ resolved = resolve_run(session, target) → ResolvedRun  resolve.py:156
  └─ render_report(report, request, printer) → ExitStatus  check_render.py:97
```

Every node carries the call-site (`binding = call(args) → ReturnType`), the
callee's doc sentence, and a clickable `file:line` (OSC 8 hyperlink into
your editor — Zed/VS Code auto-detected, `$STACKDIFF_EDITOR` or `--link`
otherwise).

## Usage

```bash
stackdiff                     # HEAD vs working tree, changed entrypoints
stackdiff main                # your branch vs main
stackdiff v1 v2 -e handle     # ref to ref, one entrypoint
stackdiff --tree -e run       # one world's call tree (no diff)
stackdiff --tree              # list entrypoints, grouped by file
stackdiff --callers -e save   # reverse: who calls save, and who calls them
```

Semantics mirror `git diff`: no refs → HEAD vs worktree; one ref → that vs
worktree; two refs → ref to ref.

## Views

| Flag | What you get |
| --- | --- |
| (default) | rich text tree with diff rails |
| `-m` / `--view boxes` | boxed graph, left→right, status-tinted, branch conditions on the arrows |
| `--view seq` | sequence diagram — lifelines per file, calls as messages, returns as replies |
| `--view er` | class diagram — types in play, their fields and touched methods |
| `--view modules` | one node per file, arrows where calls cross files |
| `--format json` | full node data for machines |
| `--format markdown` | the tree in a ```` ```diff ```` fence, paste-ready for PRs/chat |
| `--format mermaid` | mermaid source with status classes, for any mermaid renderer |

## Defaults that keep graphs readable

- depth 3 (`--max-depth N` to go deeper)
- unchanged limbs pruned to `…` with 1 context sibling (`--context N`, `--full`)
- test files excluded (`--tests`, or name a test path after `--`)
- `.gitignore` respected (`--no-ignore`)
- builtin/plumbing calls hidden — `len()`, `clone()`, `println!` (`--noise`)
- repeated subtrees shown once, then `▸ shown above` (`--no-dedupe`)
- consecutive identical calls collapse to `×N`
- output pages through `less` when it exceeds the screen (`--no-pager`)

## Per-repo tuning: `.stackdiff.toml`

```toml
# noise: hide framework plumbing, rescue false positives
hide = ["sa.*", "cast", "bindparam"]
show = ["ValueError"]

[defaults]
max_depth = 2
view = "boxes"
link = "zed"
```

Tune the noise lists from data: `stackdiff --noise-report` prints every
unresolved call by frequency, marked `hidden`/`shown`.

## Install

```bash
cargo install --path .
stackdiff --completions zsh > ~/.zfunc/_stackdiff   # shell completions
```
