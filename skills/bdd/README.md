# bdd

An agent skill for Behavior-Driven Development. The agent agrees on concrete examples in business language before it writes any test code.

## BDD and Gherkin in one minute

BDD is a way to agree on what software must do before you build it. Business and developers discuss concrete examples until they share one understanding. Gherkin records those examples in a form anyone can read:

```gherkin
Scenario: Transfer within the daily limit
  Given Alice has $430 in her checking account
  When Alice transfers $125 to Bob
  Then Alice's checking balance is $305
```

Each scenario is a specification and a test at the same time. `Given` sets the context, `When` triggers one action, `Then` checks the visible result.

## What it does

The agent follows four steps:

1. **Discover**: collect the rules, examples, and open questions for one small story.
2. **Formulate**: write the examples as Gherkin. Each scenario must pass a six-point quality gate.
3. **Automate**: connect the scenarios to code with pytest-bdd or behave. This step runs only when you ask for implementation.
4. **Refactor**: clean the glue code. The scenario language stays stable.

When you ask for a review, the agent stops after step 2. It reports findings and changes no files.

## Which framework?

Who writes the Gherkin decides:

- **Developers write it, domain experts review it**: use pytest-bdd. The scenarios live inside the pytest suite and run with the rest of the tests.
- **The business writes it**: use behave. Feature files are the entry point, so non-developers own them without touching test code.

## Files

| File                                                   | Content                                                           |
| ------------------------------------------------------ | ----------------------------------------------------------------- |
| [`SKILL.md`](SKILL.md)                                 | The workflow and the quality gate. The agent starts here.         |
| [`references/practice.md`](references/practice.md)     | Discovery: Three Amigos, Example Mapping, story slicing.          |
| [`references/gherkin.md`](references/gherkin.md)       | Gherkin syntax, worked examples, anti-patterns.                   |
| [`references/pytest-bdd.md`](references/pytest-bdd.md) | Automation inside a pytest suite. Needs pytest-bdd ≥ 8.0.         |
| [`references/behave.md`](references/behave.md)         | Automation with the standalone behave runner. Needs behave ≥ 1.3. |

Other Gherkin frameworks work too: keep the workflow, swap the automation reference.

## Install

First install loom using the readme then you can use:

```bash
loom add --skill bdd --yes
```

Or copy this folder into your agent's skills directory.
