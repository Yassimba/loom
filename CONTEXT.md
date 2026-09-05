# Skills & Tooling

The repo-level context: the shared agent skills, the design-approval family
(blueprint, callplan), and the tools they invoke.

## Language

**Callplan**:
A ±-railed plain-text promise of a change — a callstack diff as its spine,
plus ± text sections for schema/state/interface deltas — approved before any
code is built. Also names the skill that produces it.
_Avoid_: ASCII blueprint, text blueprint, callstack plan

**Stackdiff**:
The Rust CLI that prints AST-verified call trees (`--tree`) and callstack
diffs between two git worlds. A diverged port of the npm `calldiff` package.
_Avoid_: calldiff (reserve for the upstream npm tool), rust-calldiff

**Blueprint**:
The ASCII design-approval skill — three chat pictures (where it sits, the
flow, the callgraph), gated on approval.

**Promise**:
The approved design artifact a build must keep — a blueprint's three ASCII
views or an approved callplan. What verify compares built code against.

**Drift**:
Any difference between the promise and the built code's real behavior, found
at verify. Accepted drift updates the promise; the record matches reality.
_Avoid_: deviation, mismatch

**Manifest**:
The published, exact-pinned menu of tools this setup can provide. Tools
change version only when a new manifest lands on main.
_Avoid_: tool list, mise config (ambiguous with the contributor dev-env)

**Selection**:
The subset of the manifest a machine actually installs: the core block plus
the tools its user chose in the wizard. Updates refresh the selection's
pins, never its membership.
_Avoid_: installed tools, local manifest

**Vault**:
The user-selected Obsidian knowledge base that Loom creates or adopts and
uses as the working scope for wiki capabilities.
_Avoid_: wiki folder, knowledge folder

**Overlay**:
A user's own machine-local mise configuration, merged on top of the
manifest and never published. The tools counterpart of `personal/` skills.
_Avoid_: local config, custom tools

**Code binding**:
The pairing of one diagram element with one or more source ranges, carried
on the element itself as `data-code`. What makes a drawn box openable into
the code it stands for.
_Avoid_: code link (reserve for PFM's inline `path:line` badge), software
map (that is review's separate commit-addressed model)

**Anchor line**:
The verbatim source line a code binding quotes (`data-code-anchor`), used at
view time to prove the range still points at the code it was drawn from. The
same quoted line the evidence worker already reports per `file:line`.
_Avoid_: fingerprint, checksum

**Code panel**:
The annotatable view of a binding's ranges, opened by clicking a bound
element. In Guided Review it is the section's existing file card, reached
through the host's reveal channel; on the annotate surface it is a docked
panel the binding opens. Same role, two implementations.
_Avoid_: code peek (that is review's inline component), popover (that is the
hover preview)

**Walkthrough**:
The artifact a diagram-and-prose skill produces: a `.md` repo copy plus a
built `.html` with SVGs inlined, opened for annotation. Explain-code-flow's
walkthrough explains a feature; the guided review's walkthrough chapters a
changeset.
