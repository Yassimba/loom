# Types review

Which domain rules remain implicit or unenforced?

Read the [review contract](../references/review-contract.md). Establish language version, type-checker configuration, and modeling conventions. Cover all four passes; mark inapplicable ones in coverage.

## 1. Encode invariants

Trace external, persisted, IPC, and network input through parsing to consumers. Find lost validation proofs, repeated checks, and impossible boolean/nullable combinations.

- Parse into trusted values; let internal signatures carry the proof. Static types and assertions cannot replace runtime validation of untrusted input.
- Represent mutually exclusive states with unions or equivalent idioms; check exhaustive handling of closed state sets.
- Introduce domain IDs, versions, paths, or result types where they prevent a concrete mix-up or clarify a caller's contract.
- Pass required state explicitly instead of discovering optional ambient state deep in the flow.

`version = Version.parse(raw); install(version)` carries proof that `validate_version(raw); install(raw)` discards. Name each proposed type's invariant and affected consumers.

## 2. Model records

Use records where field-specific types or construction rules clarify a contract; retain mappings for dynamic keys, lookup tables, and open-ended data.

Prefer existing modeling tools. In Python, dataclasses can own internal invariants, `TypedDict` describes dict-shaped interfaces, and existing schema libraries parse external input. Freeze only where immutability preserves behavior; new validation dependencies need a benefit beyond replacing a few checks.

Account for every producer and consumer, including outside the edited function. Check serialization, equality, mutation, and public call-site compatibility.

## 3. Place behavior with its owner

Localize invariant-preserving operations with their state, following language idioms. `session.promote(message)` can replace separate message/status mutations when they form one domain operation.

Keep independent transformations as functions when privileged state access is unnecessary. File/network construction belongs on a type only when that dependency fits its responsibility. Judge locality of changes and validation, not method count.

## 4. Modernize supported spellings

After contract review, separate compatible mechanical rewrites from design changes. For Python, read [Python typing](../references/python-typing.md); otherwise follow the configured compiler and project conventions. Each recommendation must clarify a contract or remove a concrete maintenance cost.
