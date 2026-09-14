# How to set up Loom resources without Loom

Use this guide when you want Loom's skills, tools, or MCP servers without installing the `loom` command. You need Node.js for the skills installer and an installed coding agent.

## 1. Install skills

Run the Vercel skills installer and choose your agent, skills, and scope:

```bash
npx skills add Yassimba/loom
```

For a non-interactive OpenCode install:

```bash
# All public skills, available in every project
npx skills add Yassimba/loom --agent opencode --skill '*' --global --yes

# One skill, available only in the current project
npx skills add Yassimba/loom --agent opencode --skill tdd --yes
```

Replace `opencode` with another supported agent, or use `--agent '*'` for every detected agent. Update global installs with:

```bash
npx skills update --global
```

Claude Code users can install the same skills as a plugin:

```text
/plugin marketplace add Yassimba/loom
/plugin install loom@loom
```

## 2. Install command-line tools

Open the [tools list](TOOLS.md), choose the tools you need, and follow each project's installation instructions. The list includes `pup`, `sem`, `gh`, `jira`, diagram tools, and the versions tested by Loom.

Installing a command-line tool does not connect it to an account. Run its login or setup command, such as `pup auth login`, `gh auth login`, or `jira init`.

## 3. Connect MCP servers

Loom reviews two Pi servers: local Codebase Memory for repository graph/impact analysis, and hosted Context7 for library documentation. Context7 needs no local install or API key for basic use.

### OpenCode

Add Context7 to global `~/.config/opencode/opencode.json` or project `opencode.json`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "context7": {
      "type": "remote",
      "url": "https://mcp.context7.com/mcp",
      "enabled": true
    }
  }
}
```

Restart OpenCode, then run:

```bash
opencode mcp list
```

See the [OpenCode MCP guide](https://opencode.ai/docs/mcp-servers/) for authentication, headers, and per-agent tool access.

### Claude Code

Add Context7 for your user account:

```bash
claude mcp add --scope user --transport http context7 https://mcp.context7.com/mcp
claude mcp list
```

Use `--scope project` to write shared project configuration instead. See the [Claude Code MCP guide](https://docs.anthropic.com/en/docs/claude-code/mcp) for other scopes and authentication.

### Codex

Add Context7 to Codex's shared MCP configuration:

```bash
codex mcp add context7 --url https://mcp.context7.com/mcp
codex mcp list
```

Codex stores user configuration in `~/.codex/config.toml`. See the [Codex MCP guide](https://developers.openai.com/codex/mcp/) for project configuration and authentication.

### Cursor

Open **Settings > Cursor Settings > Tools & MCP**, then add Context7 as a remote server. See the [Cursor MCP guide](https://docs.cursor.com/context/model-context-protocol) for global and project configuration.

### Pi

Let Loom install the local binaries, shared adapter, and reviewed entries:

```bash
loom add --mcp-server codebase-memory-mcp --agent pi
```

Use `--scope project` for project-only server configuration. Codebase Memory runs locally over stdio with gateway-only exposure and its read-only analysis profile. Restart Pi and open `/mcp` to check the connections.

Install Context7 separately with `loom add --mcp-server context7 --agent pi`.

## 4. Install agent-specific packages

Pi packages run only in Pi. Install the package you need:

```bash
pi install npm:@yassimba/pi-fast
pi install npm:@yassimba/pi-guardrails
pi install npm:@yassimba/pi-skill-autocomplete
pi install npm:@yassimba/pi-loom-mermaid
```

Other agents still use the shared skills from step 1. They do not load Pi extensions. Check each external tool's documentation for a native plugin when you need deeper integration.
