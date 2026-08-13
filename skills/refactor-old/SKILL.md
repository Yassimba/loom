---
name: refactor-old
description: The original net-negative refactor skill, frozen while /refactor is rebuilt. Invoke by name only.
disable-model-invocation: true
---

# Refactor

Improve code structure without changing external behavior. **Net-negative** is both the goal and the measure: fewer lines in the codebase after than before — the smallest end state, not the smallest change. Writing 50 lines that delete 200 is a net win; keeping 14 functions to avoid writing 2 is a net loss. More code begets more code — entropy accumulates. A net-positive change must buy a concrete, needed invariant (better types, enforced immutability) — and the commit body must name which.

Iron laws: every "tests still pass" claim needs fresh test output; unexpected breakage gets root-caused, not patched.

## Load a Mindset

Pick the mindset that fits the target scope, read its section in [references/mindsets.md](references/mindsets.md), and open your report by naming it and its core principle.

| Mindset                | Pick when                       |
| ---------------------- | ------------------------------- |
| Simplicity vs Easy     | Untangling coupled concepts     |
| Design Is Taking Apart | Splitting a god object          |
| Data Over Abstractions | Too many custom types           |
| Happy Path First       | Mechanics or defense burying the main flow |
| PAGNI                  | Deciding what survives deletion |

Tools, loaded when the work calls for them:

| Tool                                             | Load when                                                                   |
| ------------------------------------------------ | --------------------------------------------------------------------------- |
| [references/patterns.md](references/patterns.md) | Applying a change — 10 refactoring patterns with before/after Python code   |
| [references/python.md](references/python.md)     | Target is Python — modernization smells and the exact quality-gate commands |

## What To Refactor

### Entropy and bloat

- Dead code, unused imports, unreachable branches
- Wrapper classes adding no behavior over what they wrap
- Abstractions with a single implementation (delete the abstraction, keep the implementation — unless it abstracts over a third-party library)
- Features nobody uses — delete them
- "Flexibility" that's never exercised — delete it

### AI slop

- Comments narrating what the code does — delete; a comment earns its line only by stating a *why* the code can't show
- Defensive checks in trusted paths — null checks and try/except re-verifying values a boundary already validated. Fix the root: parse once at the boundary into a trusted type, then delete the interior checks ([references/patterns.md](references/patterns.md): Parse, Don't Validate)
- Lazy typing — `Any`, casts, `# type: ignore` / `as` escapes where a precise type exists
- Machinery for theoretical failures — retries, fallback chains, lifecycle counters with no runtime evidence (a log, a reproduction, a user report). Delete; when evidence arrives, fix the smallest real failure at the boundary that owns it

### Reinvented wheels

- Hand-rolled logic the standard library already provides (`itertools`, `pathlib`, `functools`, LINQ, `Math`, …)
- Custom implementations of something a dependency already in the project does — call the dependency
- Old idioms where a modern language feature is shorter (Python specifics: [references/python.md](references/python.md))

### Structural complexity

- Functions longer than 30 lines
- Nesting deeper than 2 levels (flatten with guard clauses / early returns)
- Functions with more than 4 positional parameters

### Duplication

- Copy-paste logic across functions or modules (Rule of Three: 3rd occurrence = extract)
- Near-identical classes differing by one or two fields

### Naming and clarity

- Vague names (`data`, `result`, `info`, `handle`, `process`, `manager`, `helper`, `util`)
- Misleading names — the name promises less than the code does (a `get_` that also mutates, a `check_` that also saves); rename to what it actually does
- Abbreviations that hurt readability (`cfg`, `mgr`, `ctx` used inconsistently; well-known ones like `api`, `url`, `id`, `db` are fine)
- Single-letter variables in business logic (allowed: `i/j/k` in tight numeric loops, `x/y/z` for coordinates, math-notation in math functions)
- Booleans without an `is_`/`has_`/`can_`/`should_` prefix (`user.active` → `user.is_active`)
- Boolean flags whose call sites read ambiguously (`run(true)`)
- Magic numbers — promote to named constants with units (`time.sleep(3600)` → `ONE_HOUR_IN_SECONDS`)

### Type safety

- Bags of untyped data (`dict[str, Any]`-style) where a record type fits
- Stringly-typed dispatch (`if kind == "sql"`) — promote to an enum or protocol
- Mutually exclusive states as nullable fields plus booleans (`starting: bool, url: str | None, error: str | None`) — model the legal states as a tagged union ([references/patterns.md](references/patterns.md): Illegal States)
- Primitive obsession — IDs, versions, paths, URLs passed as raw `str`/`int` where an invariant exists → value object (net-positive, so the commit body must name the invariant it buys; Data Over Abstractions still gates it)
- Mutable collections holding data that never changes

### Coupling and cohesion

- God classes mixing I/O, business logic, orchestration
- Orchestrators owning mechanics — a use-case function containing parsing, process plumbing, or protocol details; push each below a well-named boundary so the top level reads like English ([references/patterns.md](references/patterns.md): Happy-Path Orchestration)
- Helper shrapnel — `prepare_x()` / `do_x()` / `finish_x()` shallow helpers that force readers to reconstruct one operation; inline them, or extract one deep operation
- Re-parse coupling (module A generates files, module B re-parses them — share the IR)
- Deep inheritance hierarchies where composition works

## Bias Toward Deletion

Deletion is the default; keeping is what needs justification. Three questions for every finding:

1. **What's the smallest codebase that solves this?** Not the smallest change — the smallest result. Could this be 2 functions instead of 14? Could it be 0 (delete the feature)?
2. **Is the change net-negative?** "Better organized", "more flexible", "cleaner separation" — if it's more code, it's more entropy, whatever it's called.
3. **What does this make obsolete?** Every change is a chance to delete whatever was only needed by the thing being replaced.

Deletion legitimately loses when: the codebase is already minimal for what it does; a framework's conventions demand the structure; compliance mandates it; or the code is a PAGNI ([references/mindsets.md](references/mindsets.md)) — structure that would cost 10× to retrofit (observability, auth boundaries, event schemas), worth keeping even while under-used.

## Workflow

### 1. Analyze

- Read every file in the target scope
- Sweep every "What To Refactor" category against every file. Analysis is complete only when every category is accounted for — each has findings or is explicitly reported clean.
- Severity per finding: **high** (blocks maintainability), **medium** (hurts readability), **low** (style / modernization)
- For every finding, first ask: can we DELETE this instead of fixing it?

### 2. Plan

- Order changes by dependency (data types first, consumers last)
- Identify public-API impact — any import break?
- Refactors preserve behavior; wanting a new test signals you've crossed into a behavior change — write it as a failing test first
- Measure before: `tokei <target>` — record the Code count (not comments/blanks); set the net-negative target for after. If tokei is missing, offer to install it (`brew` / `winget` / `scoop` / `cargo install tokei`)

### 3. Refactor

- **One refactoring per commit.** Small, atomic, reviewable.
- After each change, run the project's linter, type checker, and test suite (Python: gate commands in [references/python.md](references/python.md)); existing tests stay green throughout

### 4. Verify

- Full quality gate passes
- Old code is fully gone: no `_old` aliases, no `# removed` comments, no rename-only placeholders, no orphaned imports
- Measure after: `tokei <target>` again and compare Code counts — net-negative, or the commit body names the invariant the extra lines bought

## Arguments

| Invocation                           | Scope                       |
| ------------------------------------ | --------------------------- |
| `/refactor`                          | Scan full project directory |
| `/refactor branch`                   | Only files the current branch changed since the default branch (`git diff --name-only $(git merge-base HEAD <default>)..HEAD`) |
| `/refactor src/app/core/`            | Scan specific package       |
| `/refactor src/app/core/registry.py` | Scan single file            |

## Output Format

Present findings as a categorized report:

```markdown
## Refactoring Report: <target>

**Mindsets loaded:** <e.g. Simplicity vs Easy>
**Core principles applied:** <1 line each>

### High Severity

- **Entropy** — `src/app/compat.py` — entire module is dead code, 0 imports reference it

### Medium Severity

- **Type Safety** — `src/app/core/config.py:18` — `dict[str, Any]` should be a record type

### Low Severity

- **Modernization** — `src/app/types.py:5` — `Optional[str]` → `str | None`

### Categories clean

- <categories swept with zero findings>

### Proposed Changes (in dependency order)

1. Delete `src/app/compat.py` (dead code, -140 lines)
2. ...

### Line Count (tokei Code column)

- Before: <N>
- After (target): <M>
- Delta: -<K>
```

Then present via `AskUserQuestion`:

```yaml
question: "Apply refactoring plan?"
header: "Plan"
options:
  - label: "Apply all (Recommended)"
    description: "One commit per step; run quality gate after each"
  - label: "Walk step-by-step"
    description: "Confirm each step before applying"
  - label: "Skip and report"
    description: "Save the report; human partner decides"
  - label: "Revise"
    description: "I'll suggest changes to the plan"
```

Running unattended (autonomous invocation, background session): skip the question and default to "Skip and report".

## Commit Message

Each refactor commit:

```
refactor(<scope>): <what was removed or deepened>

<body: why this reduces entropy>

Lines: -<K>
```

No `Co-Authored-By`.
