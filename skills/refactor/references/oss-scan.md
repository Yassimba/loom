# Whole-repository audit

Produce a ranked report of places where a mature package deletes custom code. The cheapest wins hide in dependencies the project already declares; the biggest wins hide in modules not yet written. Changing code or architecture to enable a package is in scope for the recommendations — this is a buy-vs-build audit, not a lint pass, and it changes no code itself.

## 1. Survey

Before any dispatch, establish:

- the package/module layout and total source LOC
- every declared dependency, per package, from the ecosystem's manifests (pyproject.toml, package.json, Cargo.toml, go.mod, …)

Done when you can name each slice of the codebase, its size, and the dependencies available to it.

## 2. Inspect every slice

Partition the source into 3–5 slices of comparable size, by package or subsystem, and inspect each slice exhaustively. For every slice:

- read the actual implementations, not just names
- compare against dependencies already available to that slice
- use the [library catalog](oss-libraries.md) as a seed, not a census
- record file + line range, approximate custom LOC, what it does, candidate package(s), confidence, and estimated line reduction
- record what was checked and found clean, so it is never re-investigated

Include anything this repository makes special: a predecessor codebase, an upstream standard whose implementation is already a dependency, or an empty module whose stack is undecided. Run an already-available clone detector when the repository provides one; otherwise inspect duplication directly rather than adding tooling for the audit.

Done when every slice and clone candidate is accounted for.

## 3. Synthesize

Merge everything into one report, tiered:

1. **Existing deps underused** — swaps with no new install; name the dependency and the custom code it absorbs
2. **Small new dependencies** — each paired with the specific lines it deletes
3. **Greenfield decisions** — empty or planned modules where choosing a package now avoids writing or porting code later
4. **Bugs found along the way** — drifted vocabulary copies, inconsistent semantics between duplicate implementations, latent correctness gaps the explorers surfaced
5. **Checked and keep** — hand-rolled code that is genuinely the right call, with the reason, so nobody re-litigates it

Every item keeps its file:line anchor, package, confidence, and estimated line delta. Close with a suggested order — bugs first, mechanical swaps second, architecture decisions third — and a total realistic reduction figure honest about what stays custom. Deliver the report in chat; write it to a file only when the user names a destination.

Done when every finding appears in exactly one tier or in "checked and keep" — nothing dropped silently.

## 4. Catalog candidates

List every recommended package missing from [the catalog](oss-libraries.md), one line each in its format: the package and the hand-roll it retires. The writer adds only packages from recommendations the user approves.
