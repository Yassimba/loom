# Product

<!-- impeccable:product-schema 1 -->

## Platform

terminal (Rust CLI + ratatui TUI; not web/ios/android)

## Users

Developers who want Yassimba's curated agent setup on their machine: a
first-time installer running the one-line bootstrap, and returning users
adding a capability or updating. They read the terminal at a glance and
expect installer conventions (pick → review → install), not a dashboard.

## Product Purpose

`loom` installs exact-pinned tools (via mise), agent skills into one or more
agent trees (Claude Code, Codex, Cursor, …), Pi packages, Herdr plugins, and a
few editor settings — then keeps them updated. Success: a fresh machine is set
up in one sitting with no surprises, and `loom update` / `loom status` /
`loom init` read as one product.

## Positioning

One reviewed catalog, one pin manifest, one installer for every agent tree.
Nothing is "latest"; the plan is shown before anything runs.

## Operating Context

- Interactive wizard (`loom`, `loom setup`, `loom add`) in an 80–120 column
  terminal; mouse optional.
- Non-interactive paths: `--skill/--tool/--pi-package/--herdr-plugin`,
  `--yes`, `--dry-run`; scripted in CI (`full-install.yml`) with stdout
  captured, so plain output must stay readable without color or a TTY.
- `loom update`, `loom status`, `loom init`, `loom sync` print short reports.

## Capabilities and Constraints

- Catalog is embedded at build time (`cli/loom/setup-catalog.json`).
- Skills may depend on other skills (pulled in transitively, shown as
  "required by").
- Runtimes (mise, Pi, Herdr) are prerequisites derived from the selection —
  not user choices.
- Terminal palette only: cyan accent, green ok, yellow optional/warn, red
  error, dim for secondary text; must honor `NO_COLOR` and `TERM=dumb`.

## Brand Commitments

Name: loom. Voice: plain, short, no exclamation marks. Marks: `✓ ! ○` in
reports; checkboxes in the wizard.

## Product Principles

- Show the plan before running it; never surprise.
- One key model everywhere: arrows move, space picks, enter continues, esc
  goes back.
- Every report line names the thing and its state; failures name the fix.
- Fewer screens, denser information, consistent columns.
