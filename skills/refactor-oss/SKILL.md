---
name: refactor-oss
description: Replace hand-rolled code with the ecosystem — a scoped refactor with an approval gate, or a whole-repository multi-agent audit; both consult and grow a library catalog
disable-model-invocation: true
---

# Refactor OSS

The best code is code you don't write. Replace hand-rolled logic with what the ecosystem already ships, at either scale:

- **Scoped refactor** — `$ARGUMENTS` or the conversation names specific code: follow the steps below and apply approved changes.
- **Whole-repository audit** — neither names code, or the user says scan or audit: follow [SCAN.md](SCAN.md) — parallel explorers, a clone scan, and a ranked report. The audit changes no code.

Both scales consult the library catalog ([LIBRARIES.md](LIBRARIES.md)), obey its standing orders, and grow it with what they adopt or recommend.

## 1. Inventory

Establish what is already paid for:

- the project's declared dependencies, from the ecosystem's manifests (pyproject.toml, package.json, Cargo.toml, go.mod, …)
- the library catalog and its standing orders ([LIBRARIES.md](LIBRARIES.md))

Done when you can say, for the scope, which packages are available and which catalog entries might apply.

## 2. Propose

For each piece of hand-rolled machinery in scope, apply the catalog's standing orders ([LIBRARIES.md](LIBRARIES.md)).

Invoke the `writing-clearly-and-concisely` skill, then present a numbered list of suggestions; each item is one sentence naming the hand-roll, its replacement, and the code it deletes, followed by two fenced Markdown code blocks tagged with the code's source language:

This is what the code looks like before:

```LANGUAGE
the current code, trimmed to the lines that change
```

and after:

```LANGUAGE
the proposed code
```

Custom code that is genuinely the right call — error recovery, domain-specific semantics, code smaller than the dependency it would pull in — is listed as keep-with-reason instead of a suggestion.

Done when every hand-roll in scope appears as a suggestion or a keep-with-reason. Wait for the user's picks.

## 3. Apply And Feed Back

Apply only the picks. Append every adopted package the catalog does not yet list — one line each, in the catalog's format: the package and the hand-roll it retires.

Done when the code passes the repository's checks and tests, and every adopted package appears in the catalog.

Task / scope:
$ARGUMENTS
