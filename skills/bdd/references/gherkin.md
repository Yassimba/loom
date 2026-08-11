# Gherkin reference

Framework-agnostic syntax and style for `.feature` files. Treat Gherkin as a product specification first, runner input second — the quality gate in [SKILL.md](../SKILL.md) applies to every scenario before glue code exists.

## File structure

One `Feature` per file. Indent two spaces; comments start a line with `#`.

```gherkin
@billing
Feature: Subscription billing
  Customers should be charged according to their active plan
  so that access and revenue remain aligned.

  Rule: Active paid subscribers keep access

    Scenario: Paid subscriber can read paid article
      Given Priya has an active basic subscription
      When Priya opens the paid article "Scaling Rails"
      Then Priya can read the article
```

- **Feature** — the capability, titled in business terms. The free-form description under the title answers Why / Who / What; it never executes but shows up in reports. `Feature: AccountController GET /api/v1/accounts` is a module name, not a feature.
- **Rule** — one business rule grouping the scenarios that illustrate it. Rules keep a feature file from decaying into an unstructured scenario list.
- **Scenario** (synonym: `Example`) — one concrete example of a rule; also an executable test. 3–5 steps; when a one-line title can't state its purpose, split it.

## Steps

- `Given` — known context. Business state, not user-interaction detail.
- `When` — the triggering event or action, one per scenario.
- `Then` — the expected outcome, verified by assertion against something externally observable.
- `And` / `But` — continue the previous keyword; `*` writes list-like steps.

```gherkin
Scenario: Transfer within the daily limit
  Given Alice has $430 in her checking account
  And Alice has not transferred money today
  When Alice transfers $125 to Bob
  Then Alice's checking balance is $305
  And Bob receives a $125 transfer notification
```

## Background

Shared context every scenario in the file needs the _reader_ to know. Runs before each scenario.

- Setup that matters to business readers → `Background` or `Given`. Setup that merely starts a browser or clears a database → fixtures/hooks.
- Keep it under ~4 lines; longer means the abstraction level is wrong or the feature needs splitting.
- Vivid names ("a global administrator named Greg"), never `User A` / `Site 1`.

## Scenario Outline

Several input/output sets for one behavior — replaces copy-pasted near-identical scenarios.

```gherkin
Scenario Outline: Today is or is not Friday
  Given today is "<day>"
  When I ask whether it's Friday yet
  Then I should be told "<answer>"

  Examples:
    | day    | answer |
    | Friday | TGIF   |
    | Sunday | Nope   |
```

Each `Examples` row runs once; the outline itself never runs.

## Doc Strings and Data Tables

Multiline text or structured rows passed to a step as its final argument:

```gherkin
Given a blog post named "Random" with Markdown body
  """markdown
  # Some Title
  """

Given the following users exist:
  | name  | email             | role  |
  | Alice | alice@example.com | admin |
```

Data Tables carry key examples, never a test matrix. Bulk combinations belong in helpers and builders; a feature file drowning in data has the wrong abstraction level.

## Tags

`@tag` on a Feature, Rule, Scenario, or Examples groups, filters, and scopes hooks: `@smoke` for fast subsets, `@browser` for environment needs, `@wip` as a temporary marker cleaned before commit. Tags classify; a clear file structure organizes.

## Language

Write Gherkin in the language the domain experts speak; a `# language: nl` first line switches keywords. Translation loss beats keyword familiarity every time.

## The quality gate, expanded

The six-point gate in [SKILL.md](../SKILL.md) is the single source; these are its worked examples, keyed by gate item. **BRIEF** — Business language, Real data, Intention revealing, Essential, Focused — compresses the whole gate.

- **Gate 2, domain language.** "Imagine it's 1922": the behavior reads without computers. `When "Bob" logs in` — the field-by-field click path, row ids, endpoints, and status codes live in glue.
- **Gate 3, concrete.** Real names, dates, amounts: "Carla has a 20% summer discount expiring on 2026-08-31" — against "a user has a discount … the result is correct".
- **Gate 5, independence.** Each scenario runs alone, in any order, in parallel. Data another scenario created is a dependency bug.

Step-level rules the gate implies:

- **One thing per step.** "Given I have shades and a brand new Mustang" splits into two `Given`s; compose low-level actions in helpers, not in step text.
- **One phrasing per state.** "I am logged in" / "I have logged in" / "my session is authenticated" — pick one and reuse it; synonyms multiply step definitions and ambiguity.

## Anti-patterns

- **Procedural UI script** — typing into fields and pressing buttons step by step. Collapse to the business action.
- **Feature-coupled steps** — a steps file per feature file, phrased for that feature only. Organize glue by domain concept (`billing`, `authentication`), shared across features.
- **Steps calling steps** — reuse lives in plain functions and fixtures, never in one step invoking another's text.
- **Conditional skips** — a scenario that skips itself at runtime covers too many behaviors or lacks a controlled environment; split it and fix the root cause.
- **Deep-implementation Then** — asserting on private fields or table columns. Verify what a user or external system observes: response, message, report, visible state change.
- **Post-hoc Gherkin** — scenarios written after the code, presented as BDD. Name it acceptance automation and run real discovery next story.

## Review checklist

Scenario review is the quality gate in [SKILL.md](../SKILL.md) applied per scenario, plus the syntax-section rules above (Background length, Outline deduplication, table restraint). Glue review, framework-neutral:

- Only steps that existing scenarios use — speculative step definitions are dead weight.
- Steps organized by domain concept, reused through helpers.
- Every `Then` asserts; hooks hold technical setup only; business state dies with its scenario.
- At least one recorded failing run before the passing one.
