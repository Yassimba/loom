# Issue tracker: Beads

Issues for this repo live in the Beads graph (`.beads/issues.jsonl`), managed
with the `br` CLI. Beads is dependency-aware: issues block each other, and
triage is computed from the graph (`bv` — see the Beads section of AGENTS.md
for the `--robot-*` triage workflow and the multi-agent claim protocol).

## Conventions

- One issue per unit of work; dependencies express ordering (`br dep add
  <blocked> <blocker>`), not numbering
- A feature's spec or PRD is the body of its epic issue; child issues link to
  it via dependencies
- Labels group issues by domain; sprints/tracks come from `bv`, not folders
- `.beads/issues.jsonl` is committed — the graph travels with the repo

## When a skill says "publish to the issue tracker"

```bash
br create "<title>" -d "<body>" [--label <label>]     # one issue
br dep add <child-id> <epic-id>                       # attach to an epic
```

Create the epic first, then children, then wire dependencies.

## When a skill says "fetch the relevant ticket"

```bash
br show <id> --json          # one issue, full body and state
br ready --json              # everything actionable right now
br list --label <label>      # scoped listing
```

## Claiming and status

Follow the multi-agent claim protocol in AGENTS.md: work an issue only when
it is unclaimed or assigned to your `$BEADS_ACTOR`; claim with
`br update <id> --claim`; close with `br close <id>`.

## Wayfinding operations

Used by `/wayfinder`. The **map** is an epic issue; **children** are issues
depending on it.

- **Map**: an epic issue holding the Notes / Decisions-so-far / Fog body —
  update it with `br update <epic-id> -d "<new body>"`
- **Child ticket**: `br create` + `br dep add <child> <epic>`; record the
  ticket type (`research`/`prototype`/`grilling`/`task`) as a label
- **Blocking**: `br dep add <blocked> <blocker>` — a ticket is unblocked when
  `br ready --json` lists it
- **Frontier**: `bv --robot-next` (or `br ready --json` for the raw list)
- **Claim**: `br update <id> --claim` before any work
- **Resolve**: `br close <id>` with a closing comment holding the answer, then
  append a context pointer (gist + issue id) to the epic's Decisions-so-far
