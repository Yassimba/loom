# Library catalog

A seed for replacement candidates, not a vetted dependency list. Load only the ecosystem files relevant to the assigned source:

- Python: [standard library and packages](oss-python.md).
- JavaScript/TypeScript: [web platform and packages](oss-javascript.md).
- Other ecosystems: inspect the project's helpers, standard library, platform, and declared dependencies directly. Missing catalog coverage does not mean no candidates exist.

## Selection rules

Prefer, in order: reuse the canonical implementation already in the codebase, use stdlib or native platform features, use an existing dependency, then consider a new package with a demonstrated net benefit. Compare semantic fit and total ownership cost under the OSS topic's criteria.

Check copied upstream vocabularies, enums, and schemas against their authoritative version; drift may be a bug rather than a refactor opportunity. A transitive package is not automatically a declared dependency available for direct imports.

The writer adds approved dependencies through the project's dependency tool, keeping manifests and lockfiles consistent.

## Catalog maintenance

The OSS topic owns candidate reporting. Target-project approval does not authorize edits to the installed skill or catalog; obtain separate authorization. Group entries by ecosystem and purpose, not date. Name the package, machinery it retires, and essential constraints. Update rather than duplicate entries; prune obsolete candidates when evidence warrants it.
