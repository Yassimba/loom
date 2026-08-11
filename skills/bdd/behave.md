# behave: from scenarios to execution

A bundled automation layer — the standalone Python feature-file runner. Requires **behave ≥ 1.3** (the 1.3.x line ended the long 1.2.6 era: `Rule` keyword, tag expressions, `before_rule`/`after_rule` hooks). Install: `uv add --dev behave`.

## Project shape

The layout is a fixed convention, not configuration:

```text
features/
  billing.feature
  steps/
    billing_steps.py     # any *.py here is loaded; nested packages allowed
  environment.py         # hooks and fixtures
behave.ini               # or [tool.behave] in pyproject.toml
```

## Running — feature files are the entry point

`behave` runs the feature files directly; every scenario executes without any binding code. An unimplemented step is **reported, never silent**: behave prints the undefined step and a ready-to-paste snippet whose body is `raise NotImplementedError(u'STEP: ...')` — that output is the red signal that starts the loop. Remaining steps of the scenario skip, and the run exits nonzero.

## Step definitions

```python
# features/steps/billing_steps.py
from behave import given, when, then

@given("Alice has ${balance:d} in her checking account")
def step_impl(context, balance):
    context.account = Account("Alice", balance)

@when("Alice transfers ${amount:d} to Bob")
def step_impl(context, amount):
    context.account.transfer(amount, to="Bob")

@then("Alice's checking balance is ${expected:d}")
def step_impl(context, expected):
    assert context.account.balance == expected
```

- Matching is **keyword-scoped**: `@given` never matches a `When` step; `@step` matches any keyword. `And`/`But` inherit the preceding keyword.
- The default matcher is `parse` — typed placeholders like `{n:d}` convert automatically. `use_step_matcher("cfparse")` adds cardinality fields; `use_step_matcher("re")` switches to named-group regexes. The switch applies to definitions that follow it in the file.
- `register_type(Money=parse_money)` adds custom placeholder types to the parser.
- Registering the same pattern twice raises `AmbiguousStep` — reuse the existing step or rephrase.

All modules under `features/steps/` load globally: organize by domain concept (`billing_steps.py`, `authentication_steps.py`), shared across features.

## State: the context object

Every step and hook receives `context`, the shared state carrier. `context.account = ...` shares state between steps of one scenario.

- Context is **layered**: attributes set during a scenario are discarded when it ends; attributes set in `before_feature`/`before_all` persist at their layer. Business state belongs on the scenario layer only — parking it on feature or root layers recreates the shared-state bug.
- `context.config` exposes configuration; environment settings (base URL, tenant) load in `before_all` and ride the root layer.

## Datatables and docstrings

A step's table arrives as `context.table` (rows keyed by heading); a `"""` block arrives as `context.text`:

```python
@given("the following users exist:")
def step_impl(context):
    context.users = [dict(zip(context.table.headings, row.cells))
                     for row in context.table]
```

## Tags, hooks, fixtures, results

- Gherkin `@tags` filter runs with tag expressions: `behave --tags="@smoke and not @wip"`.
- Hooks live in `features/environment.py`: `before_all`/`after_all`, `before_feature`/`after_feature`, `before_rule`/`after_rule`, `before_scenario`/`after_scenario`, `before_step`/`after_step`, `before_tag`/`after_tag`. Technical setup only — business context stays in `Given` steps.
- Fixtures are generator functions: `@fixture` with setup before `yield`, teardown after. Activate per scenario with `use_fixture(browser_chrome, context)` in a hook, or map tags to fixtures via `use_fixture_by_tag` and a registry in `before_tag` — behave's version of "start the browser only for `@browser` scenarios".
- Step results: passed, failed (assertion or exception), undefined (snippet printed), skipped (after an earlier failure, or filtered out). Assertions decide pass/fail — a bare `return False` passes.

## The loop

1. Write the scenario; run `behave`; read the undefined-step snippets — red.
2. Paste the snippets into a domain-named steps module; run — red (`NotImplementedError`).
3. Wire steps to real behavior; assert in `Then`; run — red for the right reason.
4. Smallest implementation — green.
5. Refactor code and glue; scenario language stays stable. Next example.

## Troubleshooting

- **Steps not found at all** → steps modules outside `features/steps/`, or an import error inside one silently drops its registrations — run `behave --no-capture` and watch for tracebacks.
- **One step undefined despite a definition** → matcher mismatch: text or placeholder typo, wrong keyword decorator, or a `use_step_matcher` switch earlier in the file changed the grammar.
- **AmbiguousStep on startup** → two modules register the same pattern; deduplicate into one domain step.
- **State missing in a later step** → set on the wrong context layer, or a hook overwrote it.
- **Scenario green suspiciously fast** → leftover `NotImplementedError` snippets replaced with `pass`, or a `Then` without an assertion.

## When to consult official docs

Formatter/reporter options, parallel execution, JUnit output, Django/Flask integration recipes, or version migration from 1.2.6: <https://behave.readthedocs.io/> and the changelog at <https://github.com/behave/behave/blob/main/CHANGES.rst>.
