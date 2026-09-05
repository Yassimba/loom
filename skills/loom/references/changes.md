# Change a Loom setup

Use Loom to manage its resources. Help, status checks, and dry runs are read-only; get confirmation before commands that change the setup or start authentication.

## 1. Inspect

Check `loom --version` and establish the target: machine, repository, or wiki vault. For an existing setup, inspect relevant `loom status` output. Read the selected command's `--help` for flags, selectors, and terminal requirements.

- On native Windows, read [windows.md](windows.md) before setup, including bootstrap.
- For a wiki vault, read [wiki.md](wiki.md) before choosing Create or Adopt.
- If Loom is missing, use **Bootstrap** below, then inspect the installed CLI's help.

Done when the version, platform, target, and requested resources are known.

## 2. Preview and confirm

Choose the Loom command from its help. For setup/add, run `--dry-run` with explicit selections. Offer detected project defaults when the user has not requested specific settings. For non-interactive execution, use the command's supported `--yes` mode and explicit selections where applicable.

Show the exact command, working directory, and expected changes. Obtain confirmation for that plan; a changed command or target needs new approval.

Done when the user approves the command and its scope.

## 3. Apply and verify

Run the approved command and inspect its complete result. Check relevant status afterward. After an update, invoke `loom --version` again: the process that performed the update may still report its old version.

Account for every requested item: verified success, failure, or skip. Preserve Loom's remaining next actions in the handoff. For authentication or configuration still required by a tool, show its official command and get confirmation before running it. Investigate failures before proposing another change.

Done when every requested item has a verified result or is explicitly incomplete with a next step.

## Bootstrap

Explain that the bootstrap installs Loom and opens setup. Show the platform's command and get confirmation before running it.

macOS/Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/Yassimba/loom/main/install.sh | sh
```

Native Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/Yassimba/loom/main/install.ps1 | iex"
```

Return to **Inspect** when `loom --version` succeeds. If bootstrap fails, report its error and diagnose it before retrying.
