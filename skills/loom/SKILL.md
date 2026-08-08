---
name: loom
description: Set up or troubleshoot Yassimba's curated agent skills, Pi packages, Herdr, and Herdr plugins through the loom CLI. Use when the user asks to install this collection, configure Herdr, add one of its capabilities, update the setup, or diagnose installation problems.
---

# Setup Yassimba

Use the `loom` CLI as the single setup interface. It installs skills itself and delegates to Pi and Herdr for the rest; do not reproduce that installation logic manually.

## Start

Check whether the CLI is available (works in any shell):

```bash
loom --version
```

If the command fails or is not found, treat the CLI as not installed: explain that the bootstrap downloads a checksum-verified binary from the `Yassimba/loom` GitHub releases, then ask before running the platform command.

macOS or Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/Yassimba/loom/main/install.sh | sh
```

Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/Yassimba/loom/main/install.ps1 | iex"
```

## Workflows

- First-time setup: `loom setup`
- Add selected capabilities: `loom add`
- Check the installation: `loom doctor`
- Update installed tooling and resources: `loom update`

Without flags, `setup` and `add` open a full-screen wizard that needs a real terminal; when run from an agent, inspect `loom add --help` and pass the matching `--skill`, `--pi-package`, or `--herdr-plugin` flags instead.

Before any command that installs or updates software, show what will run and ask for confirmation. `doctor` is read-only and may run without confirmation. Report partial failures exactly as the CLI prints them; do not claim a failed resource was installed.
