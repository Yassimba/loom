---
name: loom
description: Set up Loom on a machine or repository. Use when installing or updating Loom resources, initializing a project, finishing a selected tool's authentication, handling native Windows or WSL, checking status, or repairing setup.
---

# Loom

Loom is the only installer:

- Machine: `loom setup`
- Repository: `loom init`

## Run

1. Check `loom --version`. If missing, explain the bootstrap and ask before running it:

   ```bash
   curl -fsSL https://raw.githubusercontent.com/Yassimba/loom/main/install.sh | sh
   ```

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/Yassimba/loom/main/install.ps1 | iex"
   ```

2. Use `loom setup`, `loom add`, `loom init`, `loom update`, or `loom status` for the requested boundary. Read `--help` for current selectors and flags instead of guessing them.

3. Bare `setup` and `add` need a real terminal. When acting for the user, pass explicit selectors and preview with `--dry-run`.

4. For repository defaults, offer `loom init --yes`. Use explicit choices only when the user supplied them.

5. After setup or add, report every next action from Loom. Offer each selected tool's official authentication or configuration command; explain it and ask before running it. Credentials stay with that tool.

Native Windows: before setup, read [references/windows.md](references/windows.md).

Before any install, update, init, or upstream setup command, show the exact command and get confirmation. Status checks and dry runs need no confirmation.

Done when Loom reports every requested resource or project file as successful. Repeat failed, skipped, and remaining next actions exactly.
