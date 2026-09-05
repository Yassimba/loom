<p align="center">
  <img src="assets/loom-logo.svg" width="160" height="160" alt="Loom's woven purple logo">
</p>

# Loom

Loom weaves an entire opinionated setup together for agentic engineering. One installer gives your coding agents the skills, tools, Pi packages, and project instructions used across the full engineering workflow.

Use Loom with Pi (recommended), Claude Code, Codex, OpenCode, Cursor, Grok, or any agent that reads an Agent Skills folder.

The optional [pi-loom extension](plugins/pi-loom/README.md) adds a Loom header and startup update notice to Pi. Reusable logo and icon files live in [assets/](assets/README.md).

## Start here

### 1. Install Loom

macOS or Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/Yassimba/loom/main/install.sh | sh
```

Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/Yassimba/loom/main/install.ps1 | iex"
```

The installer adds [mise](https://mise.jdx.dev), Node.js, and the `loom` command. It then opens the setup menu.

Select **Everything** for the full setup. You can also pick skill groups, tools, Pi packages, or individual items. The menu shows the full plan before it installs anything.

### 2. Check the install

```bash
loom status
```

This shows what is installed and any setup step that still needs your attention.

### 3. Set up a project

Run this once in each repository:

```bash
cd your-project
loom init
```

`loom init` can create:

- `AGENTS.md` and `CLAUDE.md` instructions
- local issue tracking with Markdown or [Beads](https://github.com/Dicklesworthstone/beads_rust)
- domain notes for the words and rules used by the project
- coding standards
- editor links that open the right source file
- a local CodeGraph index, if you installed CodeGraph

Use `loom init --yes` to accept the detected defaults without questions.

### Windows and WSL2

Native Windows works. Use WSL2 if you also want Linux-only tools such as Herdr:

```powershell
wsl --install -d Ubuntu
```

After Ubuntu starts, run the macOS/Linux install command inside it.

## The main workflow

You do not need every step for every change. Start at the first step that helps.

```text
brainstorming
    ↓
grill-with-docs
    ↓
to-spec → to-tickets
                ↓
            implement
                ↓
            e2e-test
                ↓
             release
```

### 1. Find the idea: `brainstorming`

Use this when the idea is still loose. The agent asks short questions, compares a few directions, and writes a small idea brief.

Example:

```text
/brainstorming I want imports to be easier to undo
```

Skip this step when the task is already clear.

### 2. Make the idea precise: `grill-with-docs`

Use this when the goal is clear but the rules or design are not. The agent asks one question at a time and records decisions in the project's domain notes.

For work that is too large for one agent session, use `wayfinder` instead. It puts the open questions on the issue tracker and works through them one at a time.

### 3. Write the work down: `to-spec` and `to-tickets`

`to-spec` turns the conversation into a spec. It does not restart the interview.

`to-tickets` splits the approved spec into small pieces. Each ticket delivers a working path through the product and says which earlier tickets block it.

If the project uses Beads:

- `whats-next` shows the next ready ticket without taking it.
- `next` claims the next ready ticket and starts it.

### 4. Build one ticket: `implement`

`implement` runs the build loop for one ticket:

1. `ponytail` looks for the smallest change that works.
2. `blueprint` shows the design and waits for approval.
3. `tdd` writes a failing test before the code when a useful test fits.
4. `code-review` checks the change against both the spec and the repository rules.
5. `commit` creates one clean Conventional Commits commit.

You can also run any of these skills by itself.

### 5. Prove it works: `e2e-test`

`e2e-test` starts the real app and uses it through its CLI, API, or web page. It saves output and screenshots, then checks the stored data.

A passing test suite is useful, but this step checks the full running system.

### 6. Ship it: `release`

`release` runs lint, type checks, and tests in that order. It then commits any remaining work, pushes the branch, and opens a pull request or merge request.

If a test fails for an unclear reason, it routes the problem through `diagnosing-bugs` before changing the code.

## Other useful skills

| When you need to... | Use... |
| --- | --- |
| Find the cause of a bug before fixing it | [`diagnosing-bugs`](skills/diagnosing-bugs/SKILL.md) |
| Remove code or make it simpler | [`cleanup`](skills/cleanup/SKILL.md) or [`ponytail`](skills/ponytail/SKILL.md) |
| Understand how a feature works | [`explain-code-flow`](skills/explain-code-flow/SKILL.md) |
| Understand a whole multi-repo product in one diagram page | [`system-atlas`](skills/system-atlas/SKILL.md) |
| See how call paths changed between commits | [`calldiff`](skills/calldiff/SKILL.md) |
| Walk through a large change in the browser | [`changeset-walkthrough`](skills/changeset-walkthrough/SKILL.md) |
| Review a plan or diff with comments | [`plannotator`](skills/plannotator/SKILL.md) |
| Design or improve a web interface | [`impeccable`](skills/impeccable/SKILL.md) |
| Draw a diagram | [`diagram-design`](skills/diagram-design/SKILL.md) or [`mermaid-skill`](skills/mermaid-skill/SKILL.md) |
| Build browser slides | [`frontend-slides`](skills/frontend-slides/SKILL.md) |
| Write a README or guide | [`write-readme`](skills/write-readme/SKILL.md) or [`write-documentation`](skills/write-documentation/SKILL.md) |
| Make prose shorter and clearer | [`write-simply`](skills/write-simply/SKILL.md) |
| Work with Datadog | the `dd-*` skills for logs, APM, monitors, docs, and live debugging |

Browse the full list in [`skills.sh.json`](skills.sh.json) or in the `loom add` menu.

## Add or update your setup

Open the menu again at any time:

```bash
loom add
```

Install named items without the menu:

```bash
loom add --skill tdd --yes
loom add --skill tdd --skill diagnosing-bugs --yes
loom add --tool gh --tool gitleaks --yes
loom add --pi-package add-dir --yes
```

Preview a command before it changes anything:

```bash
loom add --skill tdd --dry-run
```

Update only the items you already chose:

```bash
loom update
```

Loom uses fixed tool and package versions. `loom update` moves them only after this repository publishes new versions. It also updates registered project skill folders and Wiki Vaults without adding new capabilities.

## Set up a Pi Wiki Vault

Select **Wiki** in `loom setup` or `loom add`, or open its dedicated manager:

```bash
loom wiki
```

Create makes a new Vault. Adopt requires an existing directory with `.obsidian/`. Loom shows the exact `claude-obsidian` changed paths and applies only the reviewed plan hash. Obsidian is optional: Loom can open its official download page, but never runs Homebrew, Winget, Snap, Flatpak, or another OS package manager.

Wiki setup also installs QMD and its Vault-local skill, registers the Vault's Markdown files, and builds search embeddings. The capability screen can optionally add Feynman and the Confluence Markdown exporter; scripted setup uses `--feynman` and `--confluence`. Each Vault gets a separate named index; setup prints its `qmd --index ... query "your question"` command. The first embedding run may download a model. Repair and `loom update` refresh the index and embeddings. This is a snapshot, not a file watcher; rerun repair after editing notes to refresh search. Unregistering a Vault leaves its QMD index intact.

Wiki skills are available only inside the Vault. Loom keeps exact-pinned Python and `claude-obsidian` product code outside it, then writes project-local Pi state below ignored `.pi/`. Optional Feynman is also installed there, not globally.

```bash
cd /path/to/Vault && pi
loom wiki status
loom wiki repair /path/to/Vault
loom wiki unregister /path/to/Vault
```

`loom status` checks every registered Vault. `loom update` refreshes each surviving Vault independently. A missing Vault is reported and never recreated. Unregister removes only Loom's machine-local record; it never deletes notes. Rerun Loom on each machine to recreate `.pi/`. Vault writes require macOS, Linux, or WSL under the pinned `claude-obsidian` contract.

## Uninstall Loom-managed resources

Open the removal menu with everything Loom owns selected. Deselect anything you want to keep; Loom also keeps its required dependencies:

```bash
loom uninstall
```

Scripts can remove all owned resources without the menu, or select exact resources:

```bash
loom uninstall --all --yes
loom uninstall --skill tdd --pi-package add-dir --yes
loom uninstall --dry-run --all
```

Modified files are preserved by default. Interactive runs ask again before deleting them; non-interactive runs require `--force-modified`. Project resources are limited to the current repository, and a partial uninstall keeps the ownership ledger so the remaining resources can be removed later.

## Choose where skills go

By default, Loom finds your installed agents and adds skills to their global folders.

Use `--agent` to name an agent. Use `--scope project` to keep the skill inside the current repository:

```bash
loom add --skill tdd --agent codex --yes
loom add --skill tdd --agent codex --agent opencode --yes
loom add --skill tdd --agent claude --scope project --yes
```

When one skill calls another, Loom installs the required skill in the same place.

## Install without Loom

The `loom` command is the easiest option, but each install method also works on its own.

### Install skills with skills.sh

```bash
npx skills add Yassimba/loom
```

### Install skills in Claude Code

```text
/plugin marketplace add Yassimba/loom
/plugin install loom@loom
```

### Install Pi packages

Loom includes three Pi packages from this repository:

```bash
pi install npm:@yassimba/pi-openai-fast
pi install npm:@yassimba/pi-add-dir
pi install npm:@yassimba/pi-skill-autocomplete
```

- [`openai-fast`](plugins/openai-fast/) adds `/fast`, which requests the priority service tier for supported models.
- [`add-dir`](plugins/pi-add-dir/) adds another directory and its root instructions to the current Pi session.
- [`skill-autocomplete`](plugins/skill-autocomplete/) adds `$` skill completion anywhere in the editor.

The setup menu also offers fixed versions of Feynman, FFF search, autoresearch, MCP support, subagents, web access, chat, rewind, and Anthropic sign-in. See [`manifest/pi-packages.json`](manifest/pi-packages.json) for the current package names and versions.

## What Loom manages

- `skills/<name>/SKILL.md`: reviewed public skills
- `plugins/<name>/`: Pi packages built in this repository
- `manifest/loom.toml`: fixed versions of tools offered by the setup menu
- `manifest/tools.json`: names and help text shown for those tools
- `cli/loom/`: the Rust setup command
- `cli/loom/setup-catalog.json`: the generated list built into the command
- `.claude-plugin/`: the Claude Code marketplace package
- `drafts/`: skills that are not reviewed or published
- `personal/`: machine-specific skills that are never published

## Contributing

Install the JavaScript workspace, then run the repository checks:

```bash
npm install
npm run check
npm run audit
```

Check the Rust command separately:

```bash
cargo fmt --manifest-path cli/loom/Cargo.toml -- --check
cargo clippy --manifest-path cli/loom/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path cli/loom/Cargo.toml
```

If you change a public skill or package entry, rebuild the setup list:

```bash
npm run catalog:generate
```

For local Pi package work, use:

```bash
scripts/sync-pi-extensions.sh status
scripts/sync-pi-extensions.sh link
```

## Credits

- Several coding skills are adapted from [Matt Pocock's skills](https://github.com/mattpocock/skills).
- [`research`](skills/research/SKILL.md) is adapted from [Matt Pocock's research skill](https://github.com/mattpocock/skills).
- The `dd-*` skills are adapted from [DataDog/pup](https://github.com/DataDog/pup) v1.10.5 under Apache 2.0.
- [`impeccable`](skills/impeccable/SKILL.md) is adapted from [pbakaus/impeccable](https://github.com/pbakaus/impeccable) v4.1.2 under Apache 2.0.
- [`i-have-adhd`](skills/i-have-adhd/SKILL.md) is adapted from [ayghri/i-have-adhd](https://github.com/ayghri/i-have-adhd) v0.2.0 at `cbe69fb` under MIT.
- [`ponytail`](skills/ponytail/SKILL.md) is adapted from [DietrichGebert/ponytail](https://github.com/DietrichGebert/ponytail) v4.9.0 under MIT.
- [`diagram-design`](skills/diagram-design/SKILL.md) is adapted from [cathrynlavery/diagram-design](https://github.com/cathrynlavery/diagram-design) v2.6.12 at `4451ead` under MIT.
- [`openai-fast`](plugins/openai-fast/) is adapted from [studioarray/pi-openai-fast](https://github.com/studioarray/pi-openai-fast) at `e82ed32` under MIT.
- Pi subagents, web access, and rewind are adapted from [nicobailon's Pi packages](https://github.com/nicobailon). Anthropic sign-in is adapted from [gotgenes/pi-anthropic-auth](https://github.com/gotgenes/pi-anthropic-auth).

Each adapted package has its own `README.md` and `THIRD_PARTY_NOTICES.md` with the exact source and license.

## License

[MIT](LICENSE)
