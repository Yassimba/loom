# Skills & Tooling

The repo-level context: the shared agent skills, the design-approval family
(blueprint, lineage, callplan), and the tools they invoke.

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
The Mermaid/HTML design-approval skill — diagrams rendered to a viewer page,
gated on approval. The heavier sibling the callplan experiment tests against.

**Promise**:
The approved design artifact a build must keep — a blueprint's lineage tab or
an approved callplan. What verify compares built code against.

**Drift**:
Any difference between the promise and the built code's real behavior, found
at verify. Accepted drift updates the promise; the record matches reality.
_Avoid_: deviation, mismatch
