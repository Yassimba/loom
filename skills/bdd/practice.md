# BDD practice: Discovery

Reference for the conversation side of BDD — the part that happens before any `.feature` file. One ordering rule applies throughout: conversations beat capturing conversations, capturing beats automating.

## Discovery

Technical and non-technical stakeholders explore one small user story together, hunting rules, examples, boundaries, counterexamples, and questions.

- One small story per conversation. A story that stays unclear after 25–30 minutes is too large or under-researched — split it or park it.
- Hold discovery just before development, so details are fresh and plans can still change.
- Ask for examples before asking for implementation.
- Record unknowns explicitly instead of guessing.

Working solo (no real meeting), simulate the three perspectives below, and mark which points are inference the user must confirm.

## Three Amigos

- **Product/business** — decides scope, value, and which boundaries belong to this story.
- **Quality** — raises boundaries, failure paths, missing cases, ways the system breaks.
- **Development** — surfaces constraints, dependencies, hidden complexity, automation feasibility.

Scenario language starts as a whole-team artifact; developers and testers may pair on Gherkin later, but business review stays active.

## Example Mapping

Clarify a story fast by sorting cards:

- **Story** (yellow) — the user story under discussion.
- **Rules** (blue) — constraints and acceptance criteria.
- **Examples** (green) — concrete cases illustrating each rule.
- **Questions** (red) — unknowns and assumptions; park them, never hard-code them into the spec.
- **New stories** — discovered scope, deferred out.

Stop when every rule has at least one example and no red card blocks the story; a timebox expiring first is the too-large-story signal — split or park.

## User stories and acceptance criteria

A story is a small, valuable slice — INVEST: Independent, Negotiable, Valuable, Estimable, Small, Testable. The classic format:

```text
As an <actor>
I want <capability>
So that <benefit>
```

The format is optional; the answers are not: who benefits, what capability, why it matters, and which concrete behavior proves it done.

## Discovery output template

```markdown
## Story
As a <actor>
I want <capability>
So that <benefit>

## Rules
- Rule 1: ...

## Examples
- Example: <concrete case>
  - Given ... When ... Then ...

## Questions
- [ ] ...

## Out of scope / new stories
- ...
```
