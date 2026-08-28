---
name: loom
description: Set up or troubleshoot Yassimba's curated setup through the loom CLI — exact-pinned tools via mise, agent-scoped global or project skills, Pi packages. Use when the user asks to install this collection, add a capability for one or more agents, install a skill globally or in the current project, scaffold AGENTS.md, update or sync the setup, or diagnose installation problems.
---

# Loom

The `loom` CLI is the single setup interface: it syncs exact-pinned tools through mise, copies skills into Claude, Agents, Codex, Pi, OpenCode, Cursor, and Grok trees itself, and delegates Pi packages to Pi. Every install goes through it — the CLI is the installer.

## Start

Check whether the CLI is available:

```bash
loom --version
```

If the command fails or is not found, treat the CLI as not installed: explain that the bootstrap installs mise, syncs the pinned core (node and the loom CLI), and hands off to the guided setup — then ask before running the platform command.

macOS or Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/Yassimba/loom/main/install.sh | sh
```

Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/Yassimba/loom/main/install.ps1 | iex"
```

## Workflows

- Guided setup (bare `loom` does the same): `loom setup`
- Add capabilities by name: `loom add --tool <name> --skill <name> --pi-package <name>`
- Target skill agents: repeat `--agent claude|agents|codex|pi|opencode|cursor|grok`; omit it to use detected agents
- Choose skill scope: `--scope global|project` (default: `global`; project resolves the current Git worktree)
- Scaffold this project's `AGENTS.md` + `CLAUDE.md`: `loom init` (run inside the project; `--python`, `--rust`, and `--adhd` skip their prompts)
- Refresh every initialized project's `AGENTS.md` from the templates: `loom sync`
- Update everything — tool pins, Pi package pins, skills, projects, the CLI itself: `loom update`
- Check the installation: `loom status`

Without resource-selection flags, `setup` and `add` open a full-screen wizard that needs a real terminal. From an agent, pass resource flags plus any requested `--agent` and `--scope` values (inspect `loom add --help` for the current names), then preview safely with `--dry-run`.

Before a command that installs or updates, show what will run and ask for confirmation. `status` and `--dry-run` are read-only and may run without asking.

Done when: every requested resource is confirmed by the CLI's own output, reported verbatim — a failed or skipped resource is reported as exactly that.
