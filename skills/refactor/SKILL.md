---
name: refactor
description: Happy-path-first design — deep modules, type-driven invariants, evidence-driven complexity, patterns that pay rent. Use when the user says "refactor", "simplify", "modernize", or when a change's design should read use-case-first.
---

# Refactor

Optimize the design for the normal user flow. If the happy path is 95% of runtime behavior, it should be approximately 95% of the code readers see.

Start with context:

- inspect the existing code, callsites, data flow
- understand the real use case before choosing abstractions
- preserve good repository patterns; do not impose a generic architecture

**Reinvented wheels first:** before designing anything, check the ecosystem — hand-rolled logic yields to the stdlib, a dependency already in the project has (new) functionality that can clean up our implementation a lot or a great package for the usecase. 

The best code is code you don't write!

**Suggest first:** invoke the `write-simply` skill then propose before changing — a numbered list, each item one sentence plus a before → after snippet. The user picks; apply only the picks.

**Measure:** on Python projects, `complexipy <target>` flags too-complex functions — prime suggestion candidates. Run `tokei <modules>` before and after, and end with the per-module line delta. Offer to install a missing tool (`uvx complexipy`, `brew install tokei`).

## Design Vocabulary

Translate the intent into established software design language:

| Desired quality | Established terminology |
| --- | --- |
| Main methods read almost like English | Composed Method, intention-revealing interface, use-case orchestration |
| Orchestrators call well-named services | Application Service, Use Case Interactor, Transaction Script |
| Ugly mechanics stay below a clean interface | Information hiding, deep modules, complexity pulled downward |
| Domain logic does not contain process and network code | Functional Core / Imperative Shell, Ports and Adapters |
| Code is organized around user behavior | Vertical Slice Architecture, use-case-driven architecture, Screaming Architecture |
| Types enforce invariants | Type-driven design, making illegal states unrepresentable |
| Boundary checks produce trusted values | Parse, Don't Validate, smart constructors, refinement types |
| Constructors and assertions enforce contracts | Design by Contract, preconditions, postconditions, invariants |
| Invalid conditions leave before the normal flow | Guard clauses, fail-fast design |
| Imagined requirements do not create code | YAGNI, evolutionary design, avoid speculative generality |
| Reuse follows real repetition | Rule of Three, semantic compression |
| Helpers hide meaningful complexity | Deep rather than shallow modules, locality of behavior |
| State and behavior have one owner | Encapsulation, Tell Don't Ask, Information Expert |

When deeper design work is required, the primary sources and reading order live in [references/sources.md](references/sources.md).

## Patterns Must Pay Rent

Patterns, layers, abstractions, objects, interfaces, and files are costs. Use them only when they produce a concrete readability, maintenance, correctness, testing, or change-isolation gain larger than their cost.

- architecture must be proportional to the real problem
- a pattern name is vocabulary for a design that emerged, not a requirement to manufacture that design
- start with the smallest honest use-case implementation, often a direct Transaction Script or vertical slice
- extract a port, repository, service, value object, or module only when it owns a real invariant, hides real complexity, has multiple real implementations, removes stable duplication, or creates a proven boundary
- judge an abstraction by total system cost: implementation lines, interfaces, files, call hops, configuration, tests, and concepts a reader must learn
- an abstraction that moves ten obvious lines into five files is negative value
- a repository that only renames one database call is a shallow module, not architecture
- do not create `Controller -> Service -> Repository` chains because a diagram, framework, blog post, or pattern says they should exist
- prefer a simple direct method until real domain complexity gives the code a natural cut point

**Pattern reference:** [references/python-patterns/](references/python-patterns/README.md) holds reference implementations of the classic patterns. Check it both ways while reviewing: 
(1) something here becomes clearer with a pattern shown there; 
(2) we already use a similar pattern —> clean our implementation against the reference. 

Rent still applies.

Screaming Architecture does not mean "add architecture layers." It means the system should reveal its domain and use cases instead of its frameworks. `invoice/pay.py` can scream the use case more clearly than `controllers/`, `services/`, and `repositories/` full of pass-through methods.

DON'T apply a repository pattern to ten obvious lines:

```python
class UserController:
    def __init__(self, service: UserService):
        self.service = service

    def get(self, id: str) -> User:
        return self.service.get(id)

class UserService:
    def __init__(self, repository: UserRepository):
        self.repository = repository

    def get(self, id: str) -> User:
        return self.repository.get(id)

class UserRepository:
    def get(self, id: str) -> User:
        return database.select_user(id)
```

DO keep a simple use case simple:

```python
async def get_user(id: UserId) -> User:
    return await database.select_user(id)
```

Add a repository later only when data access becomes a meaningful domain port or hides stable complexity that callers should not know.

## 1. Happy-Path-First Orchestration

Make orchestration read almost like English:

```python
version = read_bundled_version()
await install_exact_version(version)
await verify_installed_version(version)
await restart_server()
```

Top-level methods coordinate a use case. They should call well-named domain methods, interfaces, and services. They should not contain parsing, process plumbing, protocol details, state surgery, or long validation branches.

DON'T make the orchestrator own every detail:

```python
async def update(input: str) -> None:
    if not input:
        raise ValueError("missing version")
    result = subprocess.run(["wsl", "bash", "-lc", build_script(input)], capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(result.stderr)
    installed = parse_version(run_version_command())
    if installed != input:
        raise RuntimeError("wrong version")
    kill_existing_process()
    start_process()
```

DO expose the use case and push mechanics behind deep boundaries:

```python
async def update(version: Version) -> None:
    await server.stop()
    await cli.install(version)
    await cli.require_version(version)
    await server.start()
```

## 2. Progressive Disclosure And Guard Clauses

Use progressive disclosure:

- show the happy path first
- move necessary mechanics behind narrow, strongly named boundaries
- isolate ugly platform or integration logic in the lowest-level small method that owns it
- prefer early guards, returns, assertions, and throws so invalid inputs and failed invariants leave immediately
- keep the valid path flat and linear; do not nest it inside defensive branches
- let errors reach the existing user-facing boundary unless recovery is an explicit product requirement

## 3. Deliberate Interfaces And Behavior Ownership

Design interfaces deliberately:

- names must describe domain intent, not implementation mechanics
- dependencies should be explicit and required dependencies should be impossible to omit
- prefer small cohesive interfaces over bags of callbacks, booleans, and optional behavior switches
- put state and its invariants behind one owner
- keep IO in infrastructure and integrations; keep domain decisions out of wrappers
- use classes, services, value objects, or modules when they provide real encapsulation, identity, lifecycle, or polymorphism; do not use OOP as ceremony

## 4. Type-Driven Invariants

Use the type system as design:

- make invalid states unrepresentable where practical
- parse and validate external, persisted, IPC, and network data once at the boundary
- use domain types for meaningful IDs, versions, paths, URLs, states, and results
- return values that answer the caller's actual question
- do not use `any`, loose string protocols, or nullable states when a precise type can express the contract

Make invariants executable:

- enforce invariants through constructors, schemas, value objects, branded types, required parameters, and narrow method signatures
- validate once when data enters the domain; internal methods should receive trusted values instead of repeatedly checking raw data
- use guards and assertions for conditions that must already be true at a callsite
- make required state explicit in parameters instead of reading optional ambient state deep in the flow
- make illegal combinations impossible to construct, not merely documented
- keep invariant ownership close to the type, object, or service that controls the state

DON'T validate raw values and then throw away what the check proved:

```python
validate_version(input)
await install(input)  # still a raw string
```

DO parse into a trusted domain value once:

```python
version = Version.parse(input)
await install(version)
```

DON'T represent mutually exclusive states with nullable fields and booleans:

```python
@dataclass
class Server:
    starting: bool = False
    url: str | None = None
    error: str | None = None
```

DO model the legal states directly:

```python
@dataclass(frozen=True)
class Stopped: ...

@dataclass(frozen=True)
class Starting: ...

@dataclass(frozen=True)
class Ready:
    url: ServerUrl

@dataclass(frozen=True)
class Failed:
    error: ServerError

type ServerState = Stopped | Starting | Ready | Failed
```

DON'T pass a loose bag of optional callbacks and behavior flags:

```python
create_server(start=start, stop=stop, read=read, retry=True, legacy=False)
```

DO require a cohesive interface that answers the caller's real needs:

```python
create_server(process=server_process, cli=wsl_cli)
```

## 5. Evidence Before Complexity

Be aggressively pragmatic:

- prefer one obvious path and one source of truth
- remove duplication, stale compatibility code, speculative safeguards, theoretical race handling, and fallback chains
- do not defend against theoretical or unproven edge cases; wait until a real runtime, log, test reproduction, persisted state, or user report proves the case exists
- when runtime evidence proves an edge case, fix the smallest real failure at the boundary that owns it; do not build a general defense system around one incident
- never justify complexity with "could", "might", or "what if" alone; state the observed failure and its likelihood
- do not preserve a bad interface only to avoid changing internal callsites
- do not create a helper for every line; extract only a real concept, reusable operation, or complex boundary
- prefer less code, fewer names, fewer branches, and net-negative diffs when behavior permits

DON'T add lifecycle machinery for an imagined race:

```python
attempts: dict[Id, int] = {}
# counters, stale ownership checks, retries, cleanup, and fallback paths
# added because two calls might theoretically overlap
```

DO implement the observed flow directly:

```python
await stop_server(id)
await install_cli(version)
await start_server(id)
```

When a real runtime later reports `Text file busy`, use that evidence to add the smallest owned fix: make `stopServer` await process exit before installation. Do not build a general lifecycle framework.

## 6. Proportional Failures And Flat Control Flow

Keep failures proportional:

- handle common operational failures clearly
- fail fast on broken invariants, invalid state, and failed commands
- do not bury the happy path under code for events that should not happen
- an uncommon case gets code only after concrete runtime evidence; if its fix needs substantial machinery, explain the observed failure, frequency, and complexity cost before adding it

DON'T bury the valid path in nested conditionals:

```python
if config:
    if config.enabled:
        if server.ready:
            return run(config)
return None
```

DO reject invalid conditions first and leave the valid path flat:

```python
if not config:
    return None
if not config.enabled:
    return None
assert server.ready
return run(config)
```

## 7. Deep Modules, Not Helper Shrapnel

DON'T create shallow helpers that force readers to reconstruct one operation:

```python
prepare_update()
do_update()
finish_update()
```

DO keep tightly related simple code together, or extract one deep operation whose interface hides real complexity:

```python
await cli.install_exact_version(version)
```

## 8. Encapsulation And State Ownership

DON'T ask for owned state and mutate it elsewhere:

```python
if session.status() == "pending":
    session.messages().append(message)
    session.set_status("active")
```

DO tell the owner the domain operation:

```python
session.promote(message)
```

## 9. Domain Core And Infrastructure Boundaries

DON'T leak infrastructure into domain decisions:

```python
def promote(message: Message) -> None:
    subprocess.run(["wsl.exe", ...])
    database.insert(message)
```

DO keep domain decisions in the core and IO in ports, adapters, or the imperative shell:

```python
event = session.promote(message)
await session_store.append(event)
```

## 10. Tests At Stable Boundaries

Tests should prove behavior through real boundaries. Do not test one-line helpers, duplicate implementation logic, or build large mock systems for small changes.

DON'T test the implementation sentence by sentence:

```python
assert server_id_for("Debian") == "wsl:Debian"
assert should_restart(available=False) is True
```

DO test the stable use-case boundary and observable order:

```python
await controller.update("Debian")
assert events == ["stop", "install", "verify", "start"]
```

Prune and compress the suite:

- Delete tests that prove nothing — assignment checks, mirror-the-implementation asserts, tests that cannot fail. Keep only tests that would catch a real regression.
- Harden while shrinking with property-based testing: Hypothesis in Python; elsewhere suggest the ecosystem's framework (fast-check, proptest, jqwik). Collapse hand-written case lists into properties and strengthen the surviving tests; note `hypothesis.stateful` exists, reach for it only when a stateful model is genuinely warranted.
- Collapse further with pytest fixtures, shared fixtures in `conftest.py`, and `@pytest.mark.parametrize`.

## 11. Smells With Hard Edges

- Thresholds: functions ≤30 lines, nesting ≤2, ≤4 positional parameters.
- Names: concrete (`data`, `info`, `manager`, `helper` say nothing), honest (a `get_` that also mutates is a lie), booleans read `is_`/`has_`/`can_`; magic numbers become named constants with units.
- Comments say *why* only — delete what-comments; no `# type: ignore` / cast escapes where a precise type exists.

## Completion Standard

Finish the complete change, run focused verification, delete temporary artifacts, and do one final simplification pass. The result should feel boring, obvious, typed, cohesive, and native to the codebase.

The combined style is: **happy-path-first, use-case-oriented design with deep modules, type-driven invariants, boundary isolation, and evidence-driven complexity.**

Task / scope:
$ARGUMENTS
