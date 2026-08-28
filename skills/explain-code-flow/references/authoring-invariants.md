# Authoring invariants

The editorial rules archify applies when deciding what goes into a diagram. Rendering rules omitted.

## Shape

- One obvious main path. Side branches leave the nearest main-path node.
- At most 12 primary nodes; prefer 6–12 for architecture.
- Remove low-value edges before adding any routing control.
- Supporting detail goes in cards, not in extra edges.
- The focal element the brief names is its own node, drawn in coral; it is never merged into a neighbour.
- Fan-in: several branches may share one target, but the target keeps a normal node height (≤ 96px) and each branch reaches it with its own elbow; no node is stretched to meet arrows.
- No subtitle by default; never restate the title, nodes, or cards. No legend by default.

## Nodes

- Component types: frontend, backend, database, cloud, security, messagebus, external.
- Keep external actors outside the system boundary when that is factually true.
- Never infer identity (a brand, a product) from a vague role such as "database".
- Preserve exact product names, code identifiers, commands, protocols, API paths, environment names.

## Boundaries

- Group only real ownership, trust, process, or deployment boundaries.
- Boundaries do not replace relationships.
- Region, cluster, and security-group wording alone does not turn the diagram into a deployment topology; do that only when the user asks for one and the facts are known.

## Relationship labels

Labels are semantic data. Keep a label when it carries any of:
protocol, action, direction, synchronous/asynchronous behaviour, cross-boundary mechanism.

Delete a label only when both endpoints fully imply the relationship and it carries none of the
above, and say why the deleted label was redundant. Never delete a meaningful label to pass a check.

## Per-type placement

- architecture — one left-to-right spine with short vertical branches.
- workflow — lanes express responsibility or phase, columns express progression; retries and exception returns go outside the main lane corridor.
- sequence — participants ordered by conversation role; messages own their vertical order; return/async/security variants carry meaning, not decoration. The first message inside a fragment starts at least 40px below the fragment's top edge so its label clears the `LOOP`/`OPT` guard text; two-line labels need 48px.
- dataflow — stages express transformation or custody; rows separate parallel streams; label only data contracts, classifications, or cross-boundary movement that is not obvious.
- lifecycle — main phases on the main rail; event and terminal bands beneath later phases.
