---
name: bdd
description: Behavior-Driven Development as a collaboration and specification practice, with pytest-bdd and behave as the bundled automation references. Use when defining behavior through concrete examples or acceptance criteria, writing or reviewing Gherkin .feature files, wiring pytest-bdd or behave step definitions, or checking that tests still express business behavior rather than implementation.
---

# Behavior-Driven Development

BDD builds shared understanding of a problem through concrete examples, written in business language, agreed before implementation. The examples become executable specifications; running them drives the code. The order is fixed: **conversation first, capture second, automation last**. A team running Gherkin without the conversation has test automation, not BDD.

Two automation layers are bundled: [pytest-bdd](references/pytest-bdd.md) and [behave](references/behave.md). The practice and the Gherkin carry to any stack — see [Other automation layers](#other-automation-layers).

## Preflight: does this deserve Gherkin?

Gate every "add a BDD test" impulse:

- The behavior is stable and business-meaningful — not scaffolding, framework glue, or an implementation still finding its shape.
- The scenario states a user intent or observable outcome, not internal mechanics.
- The sentence helps someone imagine a real use of this project. A scenario that reads as a natural-language translation of a technical test gets rewritten, or dropped in favor of a plain pytest test.

## Workflow

1. **Discover.** Write down Who / What / Why, the business rules, examples, and open questions — before touching a test framework. Done when every rule carries at least one positive and one negative example and every unknown is a recorded question rather than a guess. Working solo, simulate the Three Amigos using subagents (business, development, quality) and mark which points are inference needing user confirmation. Detail and templates: [practice.md](references/practice.md).
2. **Formulate.** Invoke the `write-simply` skill (via the Skill tool) — the Gherkin and the prose presenting it follow its register. Draft the scenarios — `Given` context, `When` event, `Then` observable outcome — in domain language. Ask the user for approval, then run every scenario through the quality gate below before writing any glue code. Syntax and style: [gherkin.md](references/gherkin.md).
3. **Automate** — only when the user asks for implementation. The framework is a project fact — check the declared dependencies for `behave` or `pytest-bdd` (a `features/steps/` tree with `environment.py` means behave) and join what is there. Only greenfield picks, and Gherkin authorship decides: developers write it and domain experts review → pytest-bdd inside the existing pytest suite; the business writes the feature files themselves → behave, where they run standalone. Mechanics: [pytest-bdd.md](references/pytest-bdd.md) / [behave.md](references/behave.md). Bind scenarios, watch the run fail first, write the smallest code that passes.
4. **Refactor.** Keep scenario language stable while extracting helpers and fixtures; add boundary examples and counterexamples; report the red→green evidence and unresolved questions.

A review-only request ("review this Gherkin", "is this good BDD?") stops after step 2 plus the checklist in [gherkin.md](references/gherkin.md): report findings and the smallest corrections, change no files.

## Gherkin quality gate

Every scenario passes all six before glue code:

1. **One observable behavior** — the scenario explains a rule, boundary, or outcome.
2. **Domain language** — readable with business vocabulary alone; free of URLs, selectors, table columns, class names, HTTP details.
3. **Concrete, not technical** — real roles, amounts, dates, states; never `user1`, `foo`, or "the result is correct".
4. **Clean semantics** — `Given` is context, `When` is one event, `Then` asserts an externally observable result.
5. **Short and independent** — 3–5 steps; runnable in any order; `Background`, `Examples`, and tables carry only necessary meaning.
6. **Failing wording goes back** — a scenario that needs explanation, or is expressible only through implementation detail, returns to Discovery. Rewording beats patching it in glue.

## Other automation layers

pytest-bdd and behave are reference implementations, not the definition of BDD. On another stack or language, everything except the two framework files applies unchanged: keep the workflow and the Gherkin, swap the glue layer, and pull binding mechanics from that framework's docs.
