# bdd

An agent skill for Behavior-Driven Development. The agent agrees on concrete examples in business language before it writes any test code.

## What it does

The agent follows four steps:

1. **Discover**: collect the rules, examples, and open questions for one small story.
2. **Formulate**: write the examples as Gherkin. Each scenario must pass a six-point quality gate.
3. **Automate**: connect the scenarios to code with pytest-bdd or behave. This step runs only when you ask for implementation.
4. **Refactor**: clean the glue code. The scenario language stays stable.

When you ask for a review, the agent stops after step 2. It reports findings and changes no files.

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
