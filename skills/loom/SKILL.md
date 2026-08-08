---
name: loom
description: Set up or troubleshoot Yassimba's curated setup through the loom CLI — exact-pinned tools via mise, agent skills, Pi packages. Use when the user asks to install this collection, add a capability (tool, skill, or Pi package), scaffold a project's AGENTS.md, update or sync the setup, or diagnose installation problems.
---

# Loom

The `loom` CLI is the single setup interface: it syncs exact-pinned tools through mise, copies skills into the agent trees itself, and delegates Pi packages to Pi. Every install goes through it — the CLI is the installer.

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
- Scaffold this project's `AGENTS.md` + `CLAUDE.md`: `loom init` (run inside the project; `--python` / `--rust` / `--beads` skip the prompts)
- Refresh every initialized project's `AGENTS.md` from the templates: `loom sync`
- Update everything — tool pins, Pi package pins, skills, projects, the CLI itself: `loom update`
- Check the installation: `loom doctor`

Without selection flags, `setup` and `add` open a full-screen wizard that needs a real terminal; from an agent, pass the flags above (inspect `loom add --help` for the current names) and preview any plan safely with `--dry-run`.

Before a command that installs or updates, show what will run and ask for confirmation. `doctor` and `--dry-run` are read-only and may run without asking.

Done when: every requested resource is confirmed by the CLI's own output, reported verbatim — a failed or skipped resource is reported as exactly that.
