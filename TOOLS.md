# Loom tools

Want to install tools without Loom? Pick the ones you need below and follow the installation instructions in each project’s repository. You do not need the whole list.

This page covers every tool in [Loom’s tool manifest](manifest/loom.toml), plus mise, which the installer uses to manage tool versions. The manifest holds Loom’s tested version pins; a project’s latest release may be newer. Pi extensions are listed separately in the [README](README.md#pi-packages).

## Setup and programming languages

| Tool | What it does |
| --- | --- |
| [mise](https://github.com/jdx/mise) | Installs and switches between tool versions. |
| [Loom](https://github.com/Yassimba/loom) | Installs selected skills, tools, and Pi packages. Optional if you install everything yourself. |
| [Node.js](https://github.com/nodejs/node) | Runs JavaScript tools, including Pi. |
| [Python](https://github.com/python/cpython) | Runs Python tools and the claude-obsidian scripts. |
| [Rust](https://github.com/rust-lang/rust) | Builds tools from source. Loom uses it for envx and tokei on macOS. |
| [uv](https://github.com/astral-sh/uv) | Installs Python packages and manages Python projects. |

## Coding agents and code search

| Tool | What it does |
| --- | --- |
| [Pi](https://github.com/earendil-works/pi) | A coding agent that runs in your terminal. |
| [Herdr](https://github.com/herdrdev/herdr) | Runs coding agents in separate terminal panes. Use macOS, Linux, or WSL2. |
| [sem](https://github.com/Ataraxy-Labs/sem) | Searches code and shows how a change affects other code. |
| [Gortex](https://github.com/zzet/gortex) | Maps code relationships and makes them available to Pi and Zed. |
| [RTK](https://github.com/rtk-ai/rtk) | Shortens command output so agents use fewer tokens. |

## Code review and hosting

| Tool | What it does |
| --- | --- |
| [tuicr](https://github.com/agavra/tuicr) | Reviews code changes in the terminal and exports comments. |
| [Plannotator — Loom fork](https://github.com/Yassimba/plannotator) | Reviews plans, code, and documents in a browser. This is the fork Loom installs. |
| [GitHub CLI (`gh`)](https://github.com/cli/cli) | Manages GitHub pull requests, issues, and releases from the terminal. |
| [GitLab CLI (`glab`)](https://gitlab.com/gitlab-org/cli) | Manages GitLab merge requests and issues. Its official repository is on GitLab. |
| [glab-tui](https://github.com/rcieri/glab-tui) | Browses GitLab and GitHub projects in a terminal interface. |

## Repository tools

| Tool | What it does |
| --- | --- |
| [Gitleaks](https://github.com/gitleaks/gitleaks) | Finds passwords, tokens, and other secrets committed to Git. |
| [zoxide](https://github.com/ajeetdsouza/zoxide) | Lets you jump to frequently used directories. |
| [envx](https://github.com/mikeleppane/envx) | Views and manages environment variables. Its Rust package is named `envex`. |
| [tokei](https://github.com/XAMPPRocky/tokei) | Counts lines of code by language. |
| [tokui](https://github.com/zdyxry/tokui) | Shows tokei’s code counts in an interactive terminal view. |

## Tasks and service access

| Tool | What it does |
| --- | --- |
| [Jira CLI (`jira`)](https://github.com/ankitpokhrel/jira-cli) | Views, creates, and updates Jira issues. Run `jira init` after installation. |
| [Beads (`br`)](https://github.com/Dicklesworthstone/beads_rust) | Tracks local tasks and which tasks block others. |
| [Beads viewer (`bv`)](https://github.com/Dicklesworthstone/beads_viewer) | Browses Beads tasks and their dependencies. |
| [pup](https://github.com/DataDog/pup) | Reads and manages Datadog logs, metrics, monitors, and other services. |
| [loom-teams](https://github.com/Yassimba/loom/tree/main/cli/loom-teams) | Reads Teams and Outlook calendars, exports meetings, and finds meeting times. |

## Notes and wiki tools

| Tool | What it does |
| --- | --- |
| [claude-obsidian](https://github.com/AgriciDaniel/claude-obsidian) | Gives agents skills and scripts for working with an Obsidian notes folder. Loom uses WSL2 on Windows. |
| [QMD](https://github.com/tobi/qmd) | Searches local Markdown notes by words and meaning. |
| [Confluence Markdown Exporter (`cme`)](https://github.com/Spenhouet/confluence-markdown-exporter) | Saves Confluence pages and spaces as Markdown files. |

## Browser and diagram tools

| Tool | What it does |
| --- | --- |
| [agent-browser](https://github.com/vercel-labs/agent-browser) | Lets agents open websites, fill forms, and test pages. |
| [Mermaid CLI (`mmdc`)](https://github.com/mermaid-js/mermaid-cli) | Saves Mermaid diagrams as image or PDF files. |

## MCP servers for Pi

Loom can also connect Pi to [sem](https://github.com/Ataraxy-Labs/sem) for local code search and [Context7](https://github.com/upstash/context7) for library documentation. Both use [pi-mcp-adapter](https://www.npmjs.com/package/pi-mcp-adapter). See the [MCP setup instructions](README.md#mcp-servers).

For manual Context7 setup, add `"context7": { "url": "https://mcp.context7.com/mcp", "directTools": false }` under `mcpServers` in your Pi MCP config. Basic use needs no API key; queries go to Context7’s hosted service. Follow the linked projects’ setup instructions if you need higher rate limits.

Installing a tool does not sign you into its service or connect it to an agent. Follow its setup instructions after installation. For example, GitHub CLI uses `gh auth login` and GitLab CLI uses `glab auth login`.
