# pi-guardrails

Public Pi extension providing security hooks to prevent dangerous operations. Preserve backwards compatibility where practical.

## Stack

- TypeScript in strict mode
- npm workspace managed from the repository root
- Vitest for package tests
- Biome and TypeScript from the repository root
- `@aliou/sh` for dangerous-command AST parsing

## Structure

- `src/core/` — pure guardrail checks, path rules, and shell parsing
- `src/shared/` — shared config and public event contracts
- `extensions/guardrails/` — protected-file policies and settings
- `extensions/path-access/` — workspace-boundary checks
- `extensions/permission-gate/` — dangerous-command confirmation
- `extensions/herdr/` — Herdr prompt-state adapter

Tests live beside the code they cover. Public inter-extension events belong in `src/shared/events.ts` and must be versioned when their payload is a compatibility contract.

Run `npm test --workspace @yassimba/pi-guardrails` for package tests. The repository release process uses release-please; do not add changesets or component tags manually.
