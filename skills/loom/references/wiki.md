# Wiki setup

Use this branch when the user wants a knowledge vault rather than project instructions.

Read `loom wiki --help` for current commands. Choose Create or Adopt explicitly: creating a vault and adopting existing notes are different actions. Confirm the vault path before applying either.

Loom reviews portable vault writes through `claude-obsidian`, keeps product code outside the vault, and installs Pi packages below ignored `.pi/`. Run `cd <vault> && pi` to keep wiki skills project-local.

`loom wiki unregister <path>` removes only the machine-local registry record. It does not delete the vault.

Native Windows uses WSL for vault mutations. Obsidian itself remains optional.

Return to [Apply and verify](changes.md#3-apply-and-verify) to check the requested vault operation and report any remaining setup step.

`loom wiki sessions <path>` turns on agent-session sync for a registered vault and runs it. Without a path it syncs every vault that has it on. Session copies go below Git-ignored `.raw/agent-sessions/`; never commit them, because they can contain secrets.
