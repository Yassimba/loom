---
name: loom
description: Set up Loom on a machine or repository. Use when installing or updating Loom resources, initializing a project, finishing a selected tool's authentication, handling native Windows or WSL, checking status, or repairing setup.
---

# Loom

Loom is the only installer:

- Machine: `loom setup`
- Repository: `loom init`
- Pi Wiki Vault: `loom wiki`

## Run

1. Check `loom --version`. If missing, explain the bootstrap and ask before running it:

   ```bash
   curl -fsSL https://raw.githubusercontent.com/Yassimba/loom/main/install.sh | sh
   ```

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/Yassimba/loom/main/install.ps1 | iex"
   ```

2. Use `loom setup`, `loom add`, `loom init`, `loom wiki`, `loom update`, or `loom status` for the requested boundary. Read `--help` for current selectors and flags instead of guessing them.

3. `setup`, `add`, and `update` need a real terminal unless `--yes` is passed. When acting non-interactively, pass `--yes`; also pass explicit selectors and preview setup/add with `--dry-run`.

4. For repository defaults, offer `loom init --yes`. Use explicit choices only when the user supplied them.

5. After setup or add, report every next action from Loom. Offer each selected tool's official authentication or configuration command; explain it and ask before running it. Credentials stay with that tool.

For Wiki work, use Create or Adopt explicitly. Loom reviews all portable Vault writes through `claude-obsidian`, keeps product code outside the Vault, and installs Pi packages below ignored `.pi/`. `loom wiki unregister <path>` removes only the machine-local registry record. Never describe it as deleting the Vault. Run `cd <vault> && pi` to keep wiki skills project-local. Native Windows uses WSL for Vault mutations; Obsidian remains optional.

Native Windows: before setup, read [references/windows.md](references/windows.md).

Before any install, update, init, or upstream setup command, show the exact command and get confirmation. Status checks and dry runs need no confirmation.

Done when Loom reports every requested resource or project file as successful. Repeat failed, skipped, and remaining next actions exactly.
