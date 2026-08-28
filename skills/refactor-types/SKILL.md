---
name: refactor-types
description: Refactor toward complete, modern types — PEP 695 and advanced typing, dataclass/pydantic records over string-keyed dicts, parse don't validate, illegal states unrepresentable
disable-model-invocation: true
---

# Refactor Types

Refactor the requested code so its types carry the invariants. A reader should learn the rules of the domain from the type definitions, and the type checker should enforce them.

Work the scope in four passes, in order: **modernize** the spellings, **model** the records, **deepen** the invariants, **complete** the objects.

Before proposing, read the project's `requires-python` (or equivalent) and run its type checker for the baseline — the modern rows below are gated on the Python version, and `typing_extensions` backports the newer ones.

## Suggest First

Propose before changing. Invoke the `writing-clearly-and-concisely` skill, then present a numbered list of suggestions; each item is one sentence naming the change and its concrete benefit, followed by two fenced Markdown code blocks tagged with the code's source language:

This is what the code looks like before:

```LANGUAGE
the current code, trimmed to the lines that change
```

and after:

```LANGUAGE
the proposed code
```

Wait for the user's picks; apply only the picks.

## Design Vocabulary

Translate the intent into established software design language:

| Desired quality                               | Established terminology                                       |
| --------------------------------------------- | ------------------------------------------------------------- |
| Types enforce invariants                      | Type-driven design, making illegal states unrepresentable     |
| Boundary checks produce trusted values        | Parse, Don't Validate, smart constructors, refinement types   |
| Constructors and assertions enforce contracts | Design by Contract, preconditions, postconditions, invariants |
| State and behavior have one owner             | Encapsulation, Tell Don't Ask, Information Expert             |

## Pass 1 — Modernize

Upgrade legacy spellings on sight — each row is a mechanical rewrite the type checker verifies, applied only where the project's Python version (or `typing_extensions`) allows it:

| Legacy                                     | Modern                                                           |
| ------------------------------------------ | ---------------------------------------------------------------- |
| `Optional[X]`, `Union[X, Y]`               | `X \| None`, `X \| Y`                                            |
| `typing.List`, `Dict`, `Tuple`, `Set`      | `list[...]`, `dict[...]`, `tuple[...]`, `set[...]`               |
| `T = TypeVar("T")` + `Generic[T]`          | PEP 695: `class Stack[T]:`, `def first[T](items: list[T]) -> T:` |
| `Alias = ...`, `Alias: TypeAlias = ...`    | PEP 695: `type Alias = ...`                                      |
| methods returning the class name           | `Self`                                                           |
| stringly-typed constants                   | `Literal[...]` or a `StrEnum`                                    |
| `**kwargs: Any`                            | `Unpack[SomeTypedDict]`                                          |
| decorators that forward signatures         | `ParamSpec`                                                      |
| heterogeneous variadic tuples              | `TypeVarTuple`: `*Ts`                                            |
| boolean narrowing helpers returning `bool` | `TypeIs` (or `TypeGuard`)                                        |
| duck-typed dependencies                    | `Protocol`                                                       |
| inheritance overrides left implicit        | `@override`                                                      |

Prove union handling complete with exhaustive `match` plus `assert_never` on the fall-through arm.

Reach for the advanced tools only where they make a real contract precise — a `ParamSpec` on a decorator that forwards arguments earns its place; one on a decorator that ignores them is ceremony.

## Pass 2 — Model The Records

A string-keyed dict crossing a function boundary is a record wearing a disguise. Replace every such dict with a dataclass or a pydantic model: a frozen dataclass when the data is internal, a pydantic model when it arrives from outside and needs validation, a `TypedDict` only at the rim where an external API imposes dict shape (JSON payloads, `**kwargs`).

DON'T pass structured data as a string-keyed dict:

```python
def notify(user: dict[str, str | int]) -> None:
    send(user["email"], f"Hi {user['name']}")
```

DO model the record and parse the dict once at the boundary:

```python
@dataclass(frozen=True)
class User:
    name: str
    email: Email


def notify(user: User) -> None:
    send(user.email, f"Hi {user.name}")
```

The rewrite pays immediately: typo'd keys become attribute errors the type checker catches, the value types stop being a union smeared over every key, and the record gains a home for behavior (Pass 4).

## Pass 3 — Deepen The Invariants

Use the type system as design:

- make illegal states unrepresentable: model mutually exclusive states as a union of types, and prove handling exhaustive
- parse external, persisted, IPC, and network data once at the boundary into a trusted domain value; internal functions receive trusted values, never re-check raw data
- use domain types for meaningful IDs, versions, paths, URLs, states, and results; return values that answer the caller's actual question
- enforce invariants in constructors, schemas, value objects, and narrow signatures, kept close to the owner of the state
- make required state an explicit parameter instead of optional ambient state read deep in the flow
- assert conditions that must already be true at a callsite
- give every value the most precise type that expresses the contract — `Any`, stringly protocols, and nullable grab-bags all yield to a type that says what the value is

DON'T validate raw values and then throw away what the check proved:

```python
validate_version(raw)
await install(raw)  # still a raw string
```

DO parse into a trusted domain value once:

```python
version = Version.parse(raw)
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

## Pass 4 — Complete The Objects

Make objects complete: behavior about a type lives on the type, starting with construction. Prefer ten complete objects with ten methods each over one data object orbited by a hundred free functions.

DON'T orbit a data object with free functions:

```python
def get_config(path: Path) -> Config: ...
def config_is_stale(config: Config) -> bool: ...
```

DO give the type its own constructors and behavior:

```python
class Config:
    @classmethod
    def from_path(cls, path: Path) -> Self: ...

    def is_stale(self) -> bool: ...
```

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

## Completion Standard

Done when the approved picks are applied and, within their scope, all four passes hold: every legacy typing spelling is upgraded to its modern row; no string-keyed dict crosses a function boundary as a record; every boundary parses raw data into a trusted domain value exactly once and illegal state combinations fail to construct; behavior about a type lives on the type. Run the repository's type checker and tests and report the results.

Task / scope:
$ARGUMENTS
