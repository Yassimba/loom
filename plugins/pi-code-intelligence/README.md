# Code Intelligence for Pi

A small, non-blocking Pi extension that keeps code exploration routed to the right tool:

- Codebase Memory for repository architecture, call paths, and change impact
- Pi's native reads, grep, and tests for verification

After four consecutive native `read` or `grep` calls, the extension sends one hidden reminder to the active agent. It never blocks a tool call. Using Codebase Memory clears the reminder and resets the counter.

Loom installs this package automatically with the Codebase Memory MCP server.
