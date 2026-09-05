---
name: loom
description: "Loom help: explain the setup, choose resources, install or update, configure projects or wiki vaults, and troubleshoot installation or authentication."
---

# Loom

Use the conversation to choose a path. For a bare `/loom`, ask: "What would you like to set up, change, or understand?" Ask for a platform or target directory only when it changes the next action.

## Explain or choose

Consult **Sources** for the question. Explain the relevant behavior; for a recommendation, name the resource and the tradeoff that matters to this user. Keep this path read-only. Move to **Make a change** when the user asks to apply the recommendation.

Done when the question is answered or the choice is explained, with unverified details identified.

## Troubleshoot

Get the failing command and exact error. Use read-only probes, starting with `loom --version` and relevant `loom status` output, to distinguish a missing install, PATH problem, authentication failure, or project configuration error. Consult **Sources** for the affected command or tool.

Done when evidence identifies a cause and a proposed repair, or you name the missing evidence needed to distinguish the remaining causes. Apply repairs through **Make a change**.

## Make a change

For installation, resource selection, updates, project or wiki setup, and repairs, follow [references/changes.md](references/changes.md). That procedure owns preview, confirmation, execution, and verification.

## Sources

- **Commands and resources:** use the installed CLI's `--help` and catalog/menu. Select supported flags and resource names from that output.
- **Concepts, supported agents, and architecture:** consult the [Loom README](https://github.com/Yassimba/loom#readme) and follow the links relevant to the question. A local checkout is useful evidence; identify unpublished behavior as such.
- **Updates:** use Loom's published manifest pins as the authority. The manifest is the menu; the user's selection is what gets installed. A newer upstream tool release does not mean Loom offers it yet.
- **Authentication:** use the selected tool's official instructions. Credentials remain with that tool.
