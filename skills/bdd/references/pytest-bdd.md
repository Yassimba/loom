# pytest-bdd: from scenarios to execution

A bundled automation layer — BDD glue inside an existing pytest suite. Requires **pytest-bdd ≥ 8.0** (official Gherkin parser; `Rule`, `datatable`, and `docstring` support all landed there). Install: `uv add --dev pytest pytest-bdd`.

## Project shape

```text
tests/
  features/
    billing.feature
  step_defs/
    test_billing.py      # binds scenarios + domain-specific steps
  conftest.py            # shared steps and fixtures
pyproject.toml           # or pytest.ini
```

```ini
[pytest]
bdd_features_base_dir = tests/features/
```

## Binding scenarios — pytest runs tests, not feature files

**A `.feature` file with no binding silently runs zero tests** — nothing warns about an unbound scenario. Bind every feature:

```python
from pytest_bdd import scenarios, scenario

scenarios("billing.feature")          # one test per scenario in the file

@scenario("billing.feature", "Paid subscriber keeps access")
def test_paid_access():               # explicit binding when one scenario
    """Paid subscriber keeps access."""  # needs markers or a custom name
```

After adding scenarios, confirm the collected test count grew (`pytest --collect-only -q`). `pytest --generate-missing --feature tests/features tests/step_defs` prints unbound scenarios and undefined steps with ready-to-paste stub code.

## Step definitions

Steps are decorated functions. Matching is **keyword-scoped**: a `@given` never matches a `When` step. `@step("...", type_="given")` exists for generic registration; stacking decorators aliases one function under several phrasings.

```python
from pytest_bdd import given, when, then, parsers

@given(parsers.parse("Alice has ${balance:d} in her checking account"), target_fixture="account")
def _(balance):
    return Account("Alice", balance)

@when(parsers.parse("Alice transfers ${amount:d} to Bob"))
def _(account, amount):
    account.transfer(amount, to="Bob")

@then(parsers.parse("Alice's checking balance is ${expected:d}"))
def _(account, expected):
    assert account.balance == expected
```

- `parsers.parse` uses format-spec types (`{n:d}`, `{x:f}`); `parsers.cfparse` adds cardinality fields; `parsers.re` takes named groups. A bare string matches literally.
- `converters={"field": fn}` transforms parsed strings when format specs fall short.
- Scenario Outline `<placeholders>` substitute before matching; each `Examples` row becomes a parametrized test. Untyped placeholders arrive as strings — parse with `{n:d}` or convert.

**Visibility follows pytest fixtures**: a step is found when defined in the binding test's module or in a `conftest.py` at or above it. Shared domain steps (authentication, common setup) belong in `conftest.py`; feature-specific steps stay next to their binding module — organized by domain concept, same as any glue.

## State: fixtures

Scenario state lives in pytest fixtures:

- A `@given`/`@when` returning state exposes it via `target_fixture`; later steps request it as a parameter (the `account` flow above).
- Ordinary conftest fixtures inject infrastructure (API clients, database sessions) into any step.
- Isolation is automatic: function-scoped fixtures rebuild per scenario. Session-scoped fixtures holding *business* state leak it across scenarios — keep broad scopes for infrastructure only.
- Environment configuration (base URL, tenant) is a fixture reading settings.

## Datatables and docstrings

A step with a table receives `datatable` (list of row lists, header at `[0]`); a step with a `"""` block receives `docstring` (str):

```python
@given("the following users exist:", target_fixture="users")
def _(datatable):
    return [dict(zip(datatable[0], row)) for row in datatable[1:]]
```

## Tags, hooks, results

- Gherkin `@tags` become pytest markers: filter with `pytest -m smoke`; register names in the ini file to silence unknown-marker warnings. The `pytest_bdd_apply_tag` conftest hook customizes the mapping (e.g. `@wip` → `pytest.mark.skip`).
- Hooks in conftest: `pytest_bdd_before_scenario`, `pytest_bdd_after_scenario`, `pytest_bdd_before_step`, `pytest_bdd_after_step`, `pytest_bdd_step_error`. Reach for fixtures first — hooks are for cross-cutting concerns (reporting, screenshots-on-failure), never business setup.
- Results are plain pytest results. A missing step definition raises `StepDefinitionNotFoundError`; there is no "pending" state — **an empty step body passes silently**, so stub unfinished steps with `raise NotImplementedError` and keep every `Then` asserting; a `Then` returning a boolean passes vacuously.

## The loop

Red first, from the feature side:

1. Write the scenario; bind it; run pytest. `StepDefinitionNotFoundError` names the steps still missing — red.
2. Generate or write step stubs (`NotImplementedError` bodies); run — red.
3. Wire steps to real behavior; assert in `Then`; run — red for the right reason.
4. Smallest implementation — green.
5. Refactor code and glue; scenario language stays stable. Next example.

## Troubleshooting

- **Scenario never ran** → missing `scenarios()`/`@scenario` binding; check collected count.
- **StepDefinitionNotFoundError** → step text vs. parser mismatch (quotes, placeholders, keyword type), or the step lives where the test module can't see it.
- **fixture 'x' not found** → `target_fixture` name and the requesting parameter disagree, or the producing step never ran.
- **Outline values wrong type** → untyped placeholder; use `{n:d}` or `converters`.
- **Everything green suspiciously fast** → empty step bodies or assertion-free `Then`s.

## When to consult official docs

Exact parser grammar corner cases, `pytest-bdd` version migrations, report output formats, or async step support: <https://pytest-bdd.readthedocs.io/> and the changelog at <https://github.com/pytest-dev/pytest-bdd/blob/master/CHANGES.rst>.
