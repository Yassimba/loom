# The books, condensed

Rules from *Fundamentals of Software Architecture* (2nd ed.) and *Software Architecture: The Hard
Parts* (SAHP) that Balanced Coupling leaves out.

## Placement (step 2)

A modular monolith is one **quantum**: one deployable with one database. Per-part scalability,
elasticity and fault tolerance need a separate quantum. A synchronous call from another
deployable binds both into one quantum while it runs; an asynchronous or durable one-way message
keeps them apart.

Granularity forces (SAHP ch. 7):

| Disintegrators: split it out | Integrators: keep it in |
|---|---|
| low cohesion with the rest | needs one ACID transaction with other parts |
| changes much faster than the rest | chatty workflow with other parts |
| needs its own capacity or failure isolation | shares much code with other parts |
| different security rules, or many planned variants | its data is joined or constrained with other data |
| another team with its own release rhythm | |

## Modules (step 3)

- **Entity trap**: one module per table (`CustomerManager`) gives CRUD wrappers and spreads the
  logic over the callers. Name modules after capabilities and workflows.
- **Size** (SAHP ch. 5): a module far bigger than the others hides a monolith inside the
  monolith; split it. Duplicated business logic in several modules moves into one module.
- **Flat packages**: a domain package's `__init__.py` holds only its interface (re-exports); the
  logic lives in submodules.
- **Across a seam**, share names and types only (weak connascence). Pass what the receiver needs,
  not a large object it picks two fields from (stamp coupling).
- **Reuse**: copy a small helper rather than make two modules share a volatile one.

## Data (step 5)

SAHP ch. 6, steps 1–3 (steps 4–5 are distribution):

1. group the tables into data domains;
2. give each domain its own schema;
3. give each module its own database connection and role, limited to its schema.

A table that several modules write (SAHP ch. 9): split the table per writer, delegate the writes
to one owner, or merge the writers into one module.

A module that needs another module's data (SAHP ch. 10): call the owner's interface first; a read
model fed by the owner's events is second; a shared schema is the last resort and an accepted
coupling.

## Extraction (step 8)

A module extracts by moving its package behind a network seam: its interface becomes the remote
contract and its schema moves to its own server. The usual next style is service-based
architecture: coarse domain services, often still on one database. Its cost is the integrators
that bind it; name them per module.
