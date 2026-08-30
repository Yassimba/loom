# Loom

Loom weaves a coding-agent setup into one installable collection: 50+ skills, Pi packages, and pinned tools. Its guided installer walks you through the collection, sets up anything you're missing (like Pi or Herdr), and checks that it all works.

## A working agent setup in five minutes

On macOS or Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/Yassimba/loom/main/install.sh | sh
loom status
cd your-project
loom init
```

On Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/Yassimba/loom/main/install.ps1 | iex"
loom status
Set-Location your-project
loom init
```

Choose **Recommended** in the first-time wizard for a small, editable coding workflow. Review the exact plan, install it, then start Claude, Codex, Pi, OpenCode, Cursor, Grok, or another agent that reads the selected skill tree. `loom status` verifies the selected resources; `loom init` prepares agent instructions, issue tracking, domain docs, editor links, and coding standards for the repository.

Project-scoped skill installs are registered locally. `loom update` refreshes every surviving registered project without adding skills that were not already present in each tree.

## What's inside

**Agent skills** — 70+ skills for coding agents that read a `skills/` tree (Claude Code, Codex, OpenCode, and Pi included). They're about how you work with an agent, not what it builds: test-driven development, code review, refactoring, debugging, domain modeling, docs and diagrams, and planning. The Observability group is Datadog via [pup](https://github.com/DataDog/pup) — logs, APM, monitors, live debugger, CI flakes. Install one or all of them.

**Pi packages** — extensions for the [Pi](https://github.com/badlogic/pi-mono) coding agent.

- [**openai-fast**](plugins/openai-fast/) — turn on OpenAI fast mode (the priority service tier) from inside Pi.
- **subagents**, **web-access**, **rewind**, **anthropic-auth** — installed straight from their upstream npm packages, [exact-pinned](manifest/pi-packages.json) so they update only when this repo bumps the pin.

## The engineering flow

Most of the skills chain into one loop: you start with a vague idea and end with committed code. Each step feeds the next.

1. [**brainstorming**](skills/brainstorming/SKILL.md) — no idea yet. Talk through directions until one sticks; you end with a short brief.
2. [**wayfinder**](skills/wayfinder/SKILL.md) — the idea is real but fuzzy, or too big for one session. Break the unknowns into investigation tickets and work through them until the route is clear.
3. [**grill-with-docs**](skills/grill-with-docs/SKILL.md) — the agent interviews you, hard, until the idea is a spec. ADRs and a glossary fall out for free.
4. [**grill-with-examples**](skills/grill-with-examples/SKILL.md) — for hairy business logic: pin down every rule with concrete examples. Each example becomes a test later.
5. [**to-spec**](skills/to-spec/SKILL.md) — turn the conversation into a spec on your tracker. No more questions, just writing it down.
6. [**to-tickets**](skills/to-tickets/SKILL.md) — split the spec into small tickets that each say what blocks them.
7. [**implement**](skills/implement/SKILL.md) — build it. You approve a design sketch first, then it's TDD from there.
8. [**code-review**](skills/code-review/SKILL.md) — review the diff twice over: does it follow the repo's rules, and does it do what the spec said.
9. [**refactor**](skills/refactor/SKILL.md) — clean up, with the goal that the codebase ends up smaller than it started.
10. [**e2e-test**](skills/e2e-test/SKILL.md) — actually run the app and watch it work; a green test suite isn't proof. Add [**e2e-ux-test**](skills/e2e-ux-test/SKILL.md) when a human has to like the UI too.
11. [**commit**](skills/commit/SKILL.md) — tests green, then one clean Conventional Commits commit.

Two more sit off to the side. If something breaks mid-flow, [**diagnosing-bugs**](skills/diagnosing-bugs/SKILL.md) makes you find the actual cause before anyone touches a fix. And [**writing-clearly-and-concisely**](skills/writing-clearly-and-concisely/SKILL.md) keeps the prose the loop produces (specs, commit messages, this README) readable.

You don't have to do all of it. A small, clear feature can start at step 7.

## Guided setup

### macOS and Linux

```bash
curl -fsSL https://raw.githubusercontent.com/Yassimba/loom/main/install.sh | sh
```

### Windows

For the complete toolset, use WSL2:

```powershell
wsl --install -d Ubuntu
```

Then run the macOS/Linux command above inside Ubuntu. Native Windows remains supported:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/Yassimba/loom/main/install.ps1 | iex"
```

On native Windows, setup explains which capabilities require WSL2 and hides them if you choose to continue without it.

One command does the minimum and asks about the rest: it installs [mise](https://mise.jdx.dev), syncs the core of the [tool manifest](manifest/loom.toml) (node and the `loom` CLI, exact-pinned), and drops you into the guided setup. Your own `~/.config/mise/config.toml` is never touched; use it to layer personal tools on top.

A four-step wizard follows: **Choose** is three columns — groups on the left (Everything, skills by category, tools (Pi, herdr, gh, glab, tokei/tokui, uv, and the skills' helper CLIs, every one pinned to the version Yassin runs), Pi packages, Herdr plugins, editor settings), the chosen group's items in the middle, details on the right; space picks a row or a whole group, `/` searches. **Where** asks which agents get the skills and whether the copies belong globally or to the current project (global is the default). **Review** names every destination and anything that has to install first, then **Install** runs it and ends by telling you what to try first.

Tool versions move only when a new manifest lands on this repo — `loom update` re-syncs it and refreshes everything else you installed.

Come back to it later:

```bash
loom add       # choose more capabilities
loom update    # update installed tooling and resources
loom status    # check the setup
loom add --skill tdd --herdr-plugin annotate --yes
loom add --skill tdd --agent codex --agent opencode --yes
loom add --skill tdd --agent claude --scope project --yes
```

There's also a [`loom`](skills/loom/SKILL.md) skill, so a coding agent can run this same setup for you and ask before each step.

## Install directly

You don't need the CLI — it just drives the tools below, and each one works on its own.

### Agent skills

The CLI installs skills natively. Pick Claude, Codex, Pi, OpenCode, Cursor, Grok, or the portable Agent Skills tree with repeatable `--agent` flags; choose `--scope project` for the current Git worktree, otherwise installation is global. Omitting `--agent` preserves the convenient default of using the agents already detected on the machine. Dependencies follow the selected skill into exactly the same destinations.

Global destinations are `~/.claude/skills`, `~/.agents/skills`, `~/.codex/skills`, `~/.pi/agent/skills`, `~/.config/opencode/skills`, `~/.cursor/skills`, and `~/.grok/skills`. Project destinations are `.claude/skills`, `.agents/skills` (also Codex's portable project location), `.pi/skills`, `.opencode/skills`, `.cursor/skills`, and `.grok/skills`. OpenCode's session adapter follows the same scope, landing in the corresponding `plugins` directory so Beads claims remain resumable.

```bash
loom add --skill tdd --yes     # global, for detected agents
loom add --skill tdd --agent codex --scope project --yes
loom update                    # refresh each existing copy in place
```

The repository is also on [skills.sh](https://skills.sh):

```bash
npx skills add Yassimba/loom
```

Claude Code users can get the same skills from its marketplace:

```text
/plugin marketplace add Yassimba/loom
/plugin install loom@loom
```

### Pi packages

Each Pi package installs on its own:

```bash
pi install npm:@yassimba/pi-openai-fast
pi install npm:pi-subagents
pi install npm:pi-web-access
pi install npm:pi-rewind-hook
pi install npm:@gotgenes/pi-anthropic-auth
pi install git:github.com/earendil-works/pi-chat@9adbd29b40ee27ff1decf0fc87cbe180b40924f5
```

`pi-chat` additionally needs QEMU and tmux, and therefore runs through WSL2 rather than native Windows. See the package README under [`plugins/`](plugins/) or its upstream repository for commands and configuration.

## Uninstall Loom-managed resources

Run `loom uninstall` to open the removal menu with every Loom-owned resource selected. Deselect anything you want to keep; Loom also keeps required dependencies.

For scripted removal, use exact selectors or remove everything:

```bash
loom uninstall --skill tdd --pi-package add-dir --yes
loom uninstall --all --yes
loom uninstall --all --dry-run
```

Modified files are preserved unless an interactive run confirms their deletion or a script passes `--force-modified`. Project resources are limited to the current repository, and partial removal retains the ownership ledger.

## Repository layout

- `skills/<name>/SKILL.md` — the reviewed shared skills. Category grouping lives in `skills.sh.json`.
- `plugins/<name>/` — Pi packages, each installable on its own.
- `cli/loom/` — the Rust setup CLI.
- `cli/loom/setup-catalog.json` — the generated catalog the CLI embeds.
- `.claude-plugin/` — exposes the shared skills through the Claude Code marketplace.
- `drafts/` — unreviewed skills. Not published.
- `personal/` — machine-specific skills. Never published.

## Contributing

Install the JavaScript workspace and run the repository gates:

```bash
npm install
npm run check
npm run audit
```

Check the Rust CLI separately:

```bash
cargo fmt --manifest-path cli/loom/Cargo.toml -- --check
cargo clippy --manifest-path cli/loom/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path cli/loom/Cargo.toml
```

If you change a reviewed skill or package catalog metadata, regenerate the embedded catalog:

```bash
npm run catalog:generate
```

For local Pi extension development, use `scripts/sync-pi-extensions.sh status` and `scripts/sync-pi-extensions.sh link`.

## Credits

This repo builds on other people's work:

- Several of the coding skills are adapted from [Matt Pocock's skills](https://github.com/mattpocock/skills), and the [research](skills/research/SKILL.md) skill is his, copied verbatim.
- The Datadog `dd-*` skills are copied from [DataDog/pup](https://github.com/DataDog/pup) v1.10.5 (Apache 2.0); the `pup` CLI is pinned in the tool manifest.
- The [Impeccable](skills/impeccable/SKILL.md) design skill is from [pbakaus/impeccable](https://github.com/pbakaus/impeccable) v4.1.2 (Apache 2.0).
- The [i-have-adhd](skills/i-have-adhd/SKILL.md) skill is adapted from [ayghri/i-have-adhd](https://github.com/ayghri/i-have-adhd) v0.2.0 at `cbe69fb` (MIT).
- The [Ponytail](skills/ponytail/SKILL.md) skill is from [DietrichGebert/ponytail](https://github.com/DietrichGebert/ponytail) v4.9.0 (MIT).
- The [Diagram Design](skills/diagram-design/SKILL.md) skill is from [cathrynlavery/diagram-design](https://github.com/cathrynlavery/diagram-design) v2.6.7 at `ac490fd` (MIT; bundled icon notices included).
- subagents, web-access, and rewind are [nicobailon](https://github.com/nicobailon)'s [pi-subagents](https://github.com/nicobailon/pi-subagents), [pi-web-access](https://github.com/nicobailon/pi-web-access), and [pi-rewind-hook](https://github.com/nicobailon/pi-rewind-hook); anthropic-auth is [gotgenes/pi-anthropic-auth](https://github.com/gotgenes/pi-anthropic-auth). All install pinned from upstream npm.

Each package's README and `THIRD_PARTY_NOTICES.md` record the exact upstream version. Thanks, all.

## License

[MIT](LICENSE)
