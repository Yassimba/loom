---
name: clarify-code
description: "First-read clarity for working code: rename opaque identifiers and align nearby prose while preserving behavior and architecture. Use when a newcomer cannot infer purpose from the local code."
---

# First Read

Make the requested scope self-explanatory to a reader with no prior context. This is a semantic-preserving clarity pass.

## Map Meaning

Trace the target end to end: definitions, callers, tests, inputs, and outputs. Classify each identifier in the requested scope:

- **Precise** — its meaning is clear at the use site; keep it.
- **Opaque** — the code owns it, but a reader must inspect its implementation or remember hidden context; rename it.
- **Contract** — storage, wire, CLI, environment, public API, or third-party vocabulary; preserve it or migrate it only when requested.

Build a rename map from each opaque name to its meaning, owner, and complete use set. This step is complete when every identifier in scope has one classification and every rename has a known blast radius.

## Rename Concepts, Not Words

Rename at the owning definition, then update every caller, test, type annotation, and relevant document.

- Use the project's established domain vocabulary.
- Name values for the concept they hold and functions for the result or state change they provide.
- Include a role, state, unit, or collection distinction only when the use site needs it.
- Choose the shortest name that is unambiguous at the use site; local context already supplies the rest.
- Replace an opaque tuple or dictionary with a typed record only when its structure otherwise has to be memorized.
- Change a requested public code name directly. Add a compatibility alias only when compatibility is requested.

Keep existing abstractions, control flow, validation, and error behavior in place. Report any separate design problem instead of folding it into this pass.

## Align Prose

Invoke `writing-clearly-and-concisely`. Make nearby docstrings and comments agree with the new vocabulary. Keep prose that states purpose, contract, invariant, or a necessary reason. Remove narration and correct stale claims.

Update project context only when shared vocabulary or a user-facing workflow changed. An internal rename ends at its callers and focused documentation.

## Read It Cold

Search for every old name. Run the focused checks that cover the changed seams, then the repository's required gate. Read the diff without relying on the rename map: each changed name must reveal the concept or action at its use site.

Done means the rename map is fully applied, old names are absent, contract names changed only by request, the diff preserves behavior, and all required checks pass.
