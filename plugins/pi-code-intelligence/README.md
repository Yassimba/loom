# Code Intelligence for Pi

A small, non-blocking Pi extension that keeps code exploration routed to the right tool:

- Serena for live symbols, references, diagnostics, and structural refactors
- Codebase Memory for repository architecture, call paths, and change impact
- Pi's native reads, grep, and tests for verification

After four consecutive native `read` or `grep` calls, the extension sends one hidden reminder to the active agent. It never blocks a tool call. Using Serena or Codebase Memory clears the reminder and resets the counter.

Loom installs this package automatically with either supported code-intelligence MCP server.
