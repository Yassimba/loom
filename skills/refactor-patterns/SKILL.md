---
name: refactor-patterns
description: Refactor with the classic design patterns where they pay rent, aligned against reference implementations
disable-model-invocation: true
---

# Refactor Patterns

Refactor the requested code with the classic pattern vocabulary, in both directions:

1. **Recognize** — code here becomes clearer when reshaped into a pattern from the reference library.
2. **Align** — code here already approximates a pattern; clean the implementation against the reference version.

The reference library lives in [references/python-patterns/](references/python-patterns/README.md): reference implementations of the creational, structural, behavioral, and other classic patterns. Consult it before reshaping and while aligning — start from the README index and load only the pattern file under consideration, so the library stays out of context until a specific pattern earns its place.

## Suggest First

Propose before changing. Invoke the `write-simply` skill, then present a numbered list of suggestions; each item is one sentence naming the pattern, the move (recognize or align), and the rent it pays, followed by two fenced Markdown code blocks tagged with the code's source language:

This is what the code looks like before:

```LANGUAGE
the current code, trimmed to the lines that change
```

and after:

```LANGUAGE
the proposed code
```

Wait for the user's picks; apply only the picks.

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

Screaming Architecture does not mean "add architecture layers." It means the system should reveal its domain and use cases instead of its frameworks. `invoice/pay.py` can scream the use case more clearly than `controllers/`, `services/`, and `repositories/` full of pass-through methods.

DON'T apply a repository pattern to ten obvious lines:

```python
class UserController:
    def __init__(self, service: UserService) -> None:
        self.service = service

    def get(self, user_id: str) -> User:
        return self.service.get(user_id)


class UserService:
    def __init__(self, repository: UserRepository) -> None:
        self.repository = repository

    def get(self, user_id: str) -> User:
        return self.repository.get(user_id)


class UserRepository:
    def get(self, user_id: str) -> User:
        return database.select_user(user_id)
```

DO keep a simple use case simple:

```python
async def get_user(user_id: UserId) -> User:
    return await database.select_user(user_id)
```

Add a repository later only when data access becomes a meaningful domain port or hides stable complexity that callers should not know.

## Completion Standard

Done when the approved picks are applied and: every pattern applied names the concrete rent it pays; every near-pattern in scope is either aligned with its reference implementation or left alone with a stated reason; no pattern exists solely because its name exists. Run the repository's checks and tests and report the results.

Task / scope:
$ARGUMENTS
