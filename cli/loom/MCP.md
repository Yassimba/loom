# MCP setup through Pi's gateway

Choose Serena, Codebase Memory, or Context7 under **MCP servers** in `loom add`, or use:

```sh
loom add --mcp-server serena --mcp-server codebase-memory-mcp --agent pi --scope project --dry-run
loom add --mcp-server serena --mcp-server codebase-memory-mcp --agent pi --scope project --yes
```

`--scope global` configures `~/.pi/agent/mcp.json`; project scope configures
`<repository>/.pi/mcp.json`. Loom installs selected local binaries and
`pi-mcp-adapter` before writing the MCP entries. Pi and these prerequisites are
machine-wide; only server configuration follows scope.

## Reviewed servers

- **Serena 1.7.0** provides local symbol/LSP navigation and editing. It runs
  `serena start-mcp-server` in the `ide` context, finds the project from Pi's
  working directory, adds detected Python and Rust language servers while
  preserving Serena's inferred languages, keeps its dashboard available without
  opening it, and disables Serena memories and onboarding because Loom and
  Codebase Memory own persistent project context.
- **Codebase Memory 0.10.8** provides a local repository graph and impact
  analysis. It runs with the restricted, read-only `analysis` tool profile.
- **Context7** provides hosted library documentation. Queries leave the machine;
  basic use needs no local server or API key.

Selecting Serena or Codebase Memory also installs Loom's non-blocking Code
Intelligence extension. It adds brief routing guidance and sends one hidden
reminder after four consecutive native `read` or `grep` calls; it never blocks
a tool call.

All entries set `directTools=false`, so Pi's gateway discovers tools on request.
Loom does not run either product's broad installer or let it rewrite other agent
configuration.

Run `loom init` in a repository after installing either local server. When
available, init creates Serena's project configuration, gives Codebase Memory
a fast initial index, and adds installed-tool routing to `AGENTS.md`; repeat runs
preserve all three. Restart Pi after MCP setup and
open `/mcp` to check live health. Loom reports only whether each server is
configured; install and status commands do not start or contact them.

Loom preserves compatible absolute executable paths and unrelated MCP entries.
It rejects malformed, symlinked, disabled, direct-tool, socket, URL/local-command,
or argument conflicts before writing. It records each created entry and its tool
prerequisite so uninstall removes dependents first and preserves shared tools.
Run `loom uninstall`, select the MCP server, and review the removal. Modified and
user-managed entries remain protected.

Regression checks are in `tests/mcp_cli.rs`, `tests/install_plan.rs`, and
`src/wizard/tests.rs`. They use temporary homes and stub managers; no real
package install, server startup, network request, or user configuration write
occurs.
