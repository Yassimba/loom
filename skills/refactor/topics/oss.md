# OSS review

Find hand-rolled logic that stdlib, the platform, or a mature dependency can delete. For a whole-repository scope, follow [the OSS scan](../references/oss-scan.md). Always consult [the library catalog](../references/oss-libraries.md) as a seed, not a census.

## 1. Inventory

Establish what is already paid for:

- the project's declared dependencies, from the ecosystem's manifests (pyproject.toml, package.json, Cargo.toml, go.mod, …)
- the library catalog and its standing orders ([oss-libraries.md](../references/oss-libraries.md))

Done when you can say, for the scope, which packages are available and which catalog entries might apply.

## 2. Propose

For each piece of hand-rolled machinery in scope, apply the catalog's standing orders. Report a numbered list; each item names the hand-roll, its replacement, and the code it deletes, followed by two fenced Markdown code blocks tagged with the source language:

This is what the code looks like before:

```LANGUAGE
the current code, trimmed to the lines that change
```

and after:

```LANGUAGE
the proposed code
```

Custom code that is genuinely the right call — error recovery, domain-specific semantics, code smaller than the dependency it would pull in — is listed as keep-with-reason instead of a suggestion.

Done when every hand-roll in scope appears as a suggestion or a keep-with-reason.

## 3. Catalog feedback

List every recommended package missing from the catalog, in the catalog's format: the package and the hand-roll it retires. The later writer updates the catalog for approved recommendations.

# Anti-Patterns

Over-customizing: Wrapping a library so heavily it loses its benefits
Dependency bloat: Installing a massive package for one small feature
