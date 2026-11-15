---
name: modular-monolith-design
description: >
  Modular monolith design with tach fitness functions. Use when designing a modular monolith,
  deciding whether a capability belongs inside one deployable, or encoding module boundaries as
  tach.toml rules.
argument-hint: "[path/to/requirements.md]"
---

# Modular Monolith Design

Design one deployable whose modules are as independent as services. Every boundary becomes a
**fitness function** (a check that fails the build), and every fitness function is **probed red**
(made to fail on a planted violation) before you trust it.

Sources, each read when a step names it:

- **Balanced Coupling**: call the Skill tool with `codebase-design`, then read its
  `references/balanced-coupling.md`. Use its words **module**, **interface** and **seam**.
- **The books**: [references/books.md](references/books.md).
- **tach**: [references/tach.md](references/tach.md).
- **Document templates**: [references/templates.md](references/templates.md).

Ask the user with `AskUserQuestion`, one question at a time, only when the answer changes a
boundary, an owner or a rule. A **gate** needs the user's approval before the next step.

## Steps

### 1. Read the inputs

Read the requirements (the file in `$ARGUMENTS`, or ask for its path), the Balanced Coupling
reference and `references/books.md`. For existing code, run `tach show --mermaid -o -`.

**Done when** you can name the source roots, the current tach config (or "none"), each team, and
every store (database schema, bucket, topic).

### 2. Place every capability

Weigh each capability's disintegrators against its integrators (`books.md` §Placement) and give it
one spot:

- **module**: inside the deployable;
- **package**: a version-pinned library that other deployables import;
- **deployable**: its own runtime unit.

Code that runs inside another deployable is a package.

**Done when** every capability has a spot and the force that decided it. **Gate.**

### 3. Draw the module map

Partition by domain (`books.md` §Modules). Give each module a responsibility, the requirements it
owns, a subdomain class (core / supporting / generic), an owner team and a layer: `entry` (facade
and composition root), `domain` or `platform` (adapters). The shared kernel holds ids and value
types only.

**Done when** every requirement maps to exactly one module and every module has all fields.

### 4. Assess every integration

Fill the integrations table (template) and apply the balance rule. Pick a mechanism per edge:

- **call down**: in-process, through the lower module's interface;
- **event up**: the lower module publishes, the higher module subscribes;
- **mediator**: an orchestrating module calls both, which stay unaware of each other;
- **durable or network call**: only where a runtime gives a guarantee that a plain call cannot.

Model coupling across a team border becomes contract coupling: a `contract` subpackage of DTOs.

**Done when** the table and the map hold the same edges, the graph has no cycles, and every
unbalanced volatile edge is fixed. **Gate.**

### 5. Assign data ownership

Give every store exactly one owner module and one schema per module (`books.md` §Data). A
transaction across two modules means merge them, or move the data to one owner.

**Done when** every store has one owner and no cross-module transaction is left. **Gate.**

### 6. Encode the fitness functions

Start from the baseline in `tach.md` and encode every rule from steps 3–5 with the feature its
rule table names; give each team-owned domain its own `tach.domain.toml`. Schema ownership becomes
the Postgres test in `tach.md` §Beyond imports. Log each rule in the **rule ledger**.

Write the config by hand from the design. Code that breaks a rule keeps the rule and gets a logged
exception: a `deprecated = true` edge or a `tach-ignore(<reason>)`. `tach sync` copies the code's
imports into the config, violations included, so use it only to snapshot legacy code before step 3.

**Done when** every rule has a ledger row and `tach check` and `tach check-external` exit 0.

### 7. Probe every rule red

Plant the smallest violation per ledger row with [scripts/probe.sh](scripts/probe.sh). It plants
the snippet, runs tach and undoes the change; exit 0 means red, exit 1 means the rule did not fire.

```bash
bash <skill-dir>/scripts/probe.sh src/app/billing/__init__.py "from app.ticketing._repo import load"
bash <skill-dir>/scripts/probe.sh --external src/app/billing/__init__.py "import sqlalchemy"
```

Probe edge-level rules (visibility, cycles, closed layers) in the config (`tach.md` gotcha 3).
A rule that stays green is a config bug: fix it and probe again.

**Done when** every ledger row went red with its expected message and `git status` shows only
your intended changes.

### 8. Write the documents

Write `docs/design/<YYYY-MM-DD>/architecture.md` and one `<module>/design.md` per module from the
templates. Paste the graph from `tach show --mermaid -o -`. For each module, name the integrators
that would make extraction expensive (`books.md` §Extraction).

**Done when** every module has a design doc, the ledger matches the config row for row, and every
stable imbalance you accepted is listed under "Unresolved risks".
