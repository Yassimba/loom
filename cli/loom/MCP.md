# MCP setup: Sem through Pi's gateway

Choose **MCP servers → sem** in `loom add`, or use:

```sh
loom add --mcp-server sem --agent pi --scope project --dry-run
loom add --mcp-server sem --agent pi --scope project --yes
```

`--scope global` configures `~/.pi/agent/mcp.json`; project scope configures
`<repository>/.pi/mcp.json`. Launch Pi from that repository and grant project
trust if prompted. These are Pi-owned override paths, so adapter settings do
not leak into another host's MCP configuration. Other agent adapters have not
been implemented/verified; this does not claim they cannot support MCP.

## Reviewed prerequisites and exposure

- Sem comes from [Ataraxy-Labs/sem v0.24.0](https://github.com/Ataraxy-Labs/sem/releases/tag/v0.24.0)
  through Loom's published mise manifest. Its launch command is `sem mcp`.
- `pi-mcp-adapter` is selected automatically when Pi is installed or selected.
  Pi itself comes through mise when absent. These prerequisites are machine-wide;
  only server configuration follows scope. Review shows both selections and changes.
- Existing enabled npm adapters with installed stable versions `>=2.32.1,<3.0.0`
  are preserved under the package's same-major compatibility convention. Unknown
  sources/majors, filtered/disabled packages, and project-local adapter installs
  require manual resolution, not an automatic replacement or downgrade.

New entries contain only:

```json
{"command":"sem","args":["mcp"],"directTools":false}
```

**Lazy tool exposure is the requirement**, not a guarantee about process startup.
The `mcp` gateway discovers individual tools on request. Loom does not set
`lifecycle`, modify the adapter cache, or offer a lifecycle TUI choice. Adapter
2.32.1's `init.ts` bootstraps metadata for enabled servers when its global cache
is absent; this first-launch discovery is acceptable. No upstream fix is needed.
Compatible preexisting entries retain explicit lifecycle and all other settings.

Restart Pi after setup and open `/mcp` to check live health. Loom reports
**configured; live health not checked** and never starts Sem during installation,
status inspection, or tests. Existing direct-tool exposure, disabled entries,
custom commands, or same-name entries in another scope are conflicts to resolve
manually rather than silently overwritten.

## Safety, ownership, updates

Preflight reads the destination, shared config layers, Pi package settings, and
ownership ledger before any install lane runs, then repeats before writing. It
rejects malformed files, symlinked paths, host-import/discovery setups it cannot
verify, conflicting Sem entries, and an explicitly disabled gateway. Custom
`PI_CODING_AGENT_DIR` is not supported by this first slice. Project installs reject
`PI_MCP_CONFIG_MODE=exclusive`, which ignores project configuration; unset it or
choose global scope. Arbitrary SDK-supplied configurations and `--mcp-config` are
outside Loom's default-file setup.

Merges preserve unrelated JSON values and top-level comments, including the
adapter's active `mcp-servers` alias when `mcpServers` is absent or null. Pending
replacement backups are inspected read-only during planning and restored before
merging or removal. Existing compatible entries are no-ops and are not adopted. Modified files receive unique sibling
`.mcp.json.loom-backup-*` backups before atomic replacement. On Unix both backups
and newly written files use mode 0600; Windows uses the directory's inherited
ACLs. Backups can contain existing credentials: keep them private.

Loom records only the created entry's path, name and hash, never config contents
or credentials. Run `loom uninstall` from the configured project (or any directory
for global scope), select only **Sem** under **MCP servers**, and review the removal.
This removes only an unchanged owned entry and leaves shared prerequisites and
other entries intact. There is no scripted MCP selector; `--all` also selects other
owned resources and is not a Sem-only shortcut. Modified entries
remain protected even with `--force-modified`. An interrupted process between
config commit and receipt commit can leave an unowned entry; retry preserves it
rather than claiming user data. Ordinary receipt-save errors remove the newly
created entry again. Skill-ledger mutations wait for the Pi/MCP lane to finish,
so mixed skill/MCP installs cannot overwrite one another's receipts.

`loom update` refreshes Sem's selected mise pin without changing the selection.
MCP config and the existing adapter are left alone. Although the adapter is also
listed in the Pi-package catalog, updates exclude it from generic reinstalls to
prevent accidental downgrades. Manage gateway upgrades explicitly through Pi.
Status checks recorded entries and prerequisite presence without claiming a live
MCP handshake. User-managed entries remain user-managed.

## Verification sources

Pi 0.85.1 documentation: `packages.md`, `settings.md`, and
`environment-variables.md` (global/project package paths, filtering, trust,
version pinning and environment overrides). Adapter 2.32.1 README and published
source: Pi-owned override precedence, per-server `directTools`, and first-start
metadata bootstrap. Sem's v0.24.0 README and published release establish the
portable `sem mcp` identity; machine-local executable paths are not distributed.

Regression checks are in `tests/mcp_cli.rs`, `tests/install_plan.rs`, and
`src/wizard/tests.rs`. They use temporary homes and stub managers; no real
package install, server startup, or user configuration write was performed.
