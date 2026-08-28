# Whole-repository audit

Produce a ranked report of places where a mature package deletes custom code. The cheapest wins hide in dependencies the project already declares; the biggest wins hide in modules not yet written. Changing code or architecture to enable a package is in scope for the recommendations — this is a buy-vs-build audit, not a lint pass, and it changes no code itself.

## 1. Survey

Before any dispatch, establish:

- the package/module layout and total source LOC
- every declared dependency, per package, from the ecosystem's manifests (pyproject.toml, package.json, Cargo.toml, go.mod, …)

Done when you can name each slice of the codebase, its size, and the dependencies available to it.

## 2. Fan out

Partition the source into 3–5 slices of comparable size, by package or subsystem. Dispatch one read-only explorer subagent per slice, all in parallel, each with a fresh context and instructed to search exhaustively. Each prompt carries:

- the exact directories in scope, with the instruction to read the actual implementations, not just names
- the full list of dependencies already available to that slice
- the library catalog ([LIBRARIES.md](LIBRARIES.md)) as seed material, framed exactly as the catalog frames itself: a seed, not a census — hunt beyond it, and its standing orders apply to every slice
- the finding shape: file + line range, approximate custom LOC, what it does, candidate package(s), confidence the package genuinely covers the need, estimated line reduction
- rank findings by impact, and also report what was checked and found clean, so it is never re-investigated

Also seed each explorer with anything this particular repo makes special: a predecessor codebase to compare against, a standard whose reference implementation is already a dependency, an empty module whose stack is still undecided.

While explorers run, run a duplication scan yourself with a clone detector that fits the language (jscpd, pyscn, PMD CPD, …) — duplicated code marks exactly where shared machinery is missing, independent of any package.

Done when every slice is covered by exactly one explorer report and the clone summary is in hand.

## 3. Synthesize

Invoke the `writing-clearly-and-concisely` skill, then merge everything into one report, tiered:

1. **Existing deps underused** — swaps with no new install; name the dependency and the custom code it absorbs
2. **Small new dependencies** — each paired with the specific lines it deletes
3. **Greenfield decisions** — empty or planned modules where choosing a package now avoids writing or porting code later
4. **Bugs found along the way** — drifted vocabulary copies, inconsistent semantics between duplicate implementations, latent correctness gaps the explorers surfaced
5. **Checked and keep** — hand-rolled code that is genuinely the right call, with the reason, so nobody re-litigates it

Every item keeps its file:line anchor, package, confidence, and estimated line delta. Close with a suggested order — bugs first, mechanical swaps second, architecture decisions third — and a total realistic reduction figure honest about what stays custom. Deliver the report in chat; write it to a file only when the user names a destination.

Done when every explorer finding appears in exactly one tier or in "checked and keep" — nothing dropped silently.

## 4. Grow the catalog

Append to [LIBRARIES.md](LIBRARIES.md) every package the report recommends that the catalog does not yet list — one line each, in the catalog's format: the package and the hand-roll it retires. Add ecosystem sections as new languages come up.

Done when every package named in the report appears in the catalog.
