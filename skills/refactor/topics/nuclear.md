# Thermo-Nuclear Code Quality Review

Use this review for an unusually strict review focused on implementation quality, maintainability, abstraction quality, and codebase health.

Above all, push the reviewer to be ambitious about code structure. Do not merely identify local cleanup opportunities. Actively search for code-judo moves: restructurings that preserve behavior while making implementation dramatically simpler, smaller, more direct, and more elegant.

## Core Prompt

Start from this baseline:

> Perform a deep code quality audit of the current branch's changes.
> Rethink how to structure and implement the changes to improve code quality without impacting behavior.
> Improve abstractions and modularity. Reduce spaghetti code. Improve succinctness and legibility.
> Be ambitious. If a clear restructuring path improves the codebase, use it.
> Be thorough and rigorous. Measure twice, cut once.

## Non-negotiable standards

### Structural simplification

- Look for ways to remove whole branches, helpers, modes, conditionals, or layers.
- Prefer solutions that make the design feel inevitable in hindsight.
- Prefer deleting complexity over moving complexity.

### File size

- Treat a file growing from under 1,000 lines to over 1,000 lines as a strong smell.
- Prefer extracting helpers, subcomponents, modules, or local abstractions.
- Ask whether the file should be decomposed before accepting the change.

### Spaghetti growth

- Reject ad-hoc conditionals, scattered special cases, and one-off branches in unrelated flows.
- Move complex logic into a dedicated abstraction, helper, state machine, policy object, or module.
- Call out changes that make surrounding code harder to reason about, even when behavior works.

### Direct maintainable code

- Prefer direct, boring code over hacky or magical code.
- Flag thin abstractions, identity wrappers, and pass-through helpers that add indirection without clarity.
- Question unnecessary optionality, `unknown`, `any`, and cast-heavy code.
- Prefer explicit typed models and shared contracts.
- Make unclear invariants explicit instead of hiding them behind silent fallback.

### Boundaries and orchestration

- Keep feature logic in its canonical layer.
- Reuse existing canonical utilities instead of adding near-duplicates.
- Flag independent work serialized without a clear reason.
- Flag related updates that can leave state partially applied.

## Review questions

For every meaningful change, ask:

- Is there a code-judo move that would make this dramatically simpler?
- Can fewer concepts, branches, or helper layers express the change?
- Does the change improve or worsen local architecture?
- Did the diff add branching complexity where a better abstraction should exist?
- Did a cohesive module become more coupled, stateful, or difficult to scan?
- Is logic in the correct file and layer?
- Did a file cross a healthy size boundary?
- Do repeated conditionals signal a missing model or helper?
- Is the implementation direct and legible?
- Does the abstraction earn its keep?
- Did the change obscure invariants with casts, optionality, or ad-hoc object shapes?
- Is orchestration simpler and more atomic than required?

## Findings to flag aggressively

- Complicated implementation where reframing could delete categories of complexity.
- Refactors that move complexity without reducing it.
- Files crossing 1,000 lines because of the change.
- New conditionals bolted onto unrelated paths.
- One-off booleans, nullable modes, or flags that complicate control flow.
- Feature logic leaking into general-purpose modules.
- Generic magic handling that hides simple structure.
- Thin wrappers or identity abstractions.
- Unnecessary casts, `any`, `unknown`, or optional parameters.
- Copy-pasted logic instead of extracted helpers.
- Narrow edge-case handling inside busy functions.
- Temporary branching likely to become permanent debt.
- Bespoke helpers where a canonical utility already exists.
- Logic added in the wrong layer.
- Avoidable sequential async flow.
- Partial-update logic that weakens atomicity.

## Preferred remedies

- Delete indirection instead of polishing it.
- Reframe the state model so conditionals disappear.
- Change ownership boundaries so the feature extends the correct abstraction.
- Turn special cases into a simpler default flow.
- Extract a focused helper or pure function.
- Split large files into focused modules.
- Put feature logic behind a dedicated abstraction.
- Replace condition chains with typed models or explicit dispatch.
- Separate orchestration from business logic.
- Collapse duplicate branches.
- Delete wrappers that do not clarify the API.
- Reuse canonical helpers.
- Make type boundaries explicit.
- Parallelize independent work when it simplifies orchestration.
- Make related updates atomic when partial state is difficult to reason about.

Do not reduce structural findings to rename suggestions. Do not accept a cleaner version of the same messy idea when a simpler design is visible.

## Review tone

Be direct, serious, and demanding. Do not soften major maintainability issues into mild suggestions.

Useful phrases:

- `this pushes the file past 1k lines. can we decompose this first?`
- `this adds another special-case branch into an already busy flow. can we move this behind its own abstraction?`
- `this works, but it makes the surrounding code more spaghetti. let's keep the behavior and restructure the implementation.`
- `this feels like feature logic leaking into a shared path. can we isolate it?`
- `this abstraction seems unnecessary. can we keep the direct flow?`
- `why does this need a cast or optional here? can we make the boundary explicit instead?`
- `this looks like a bespoke helper for something we already have elsewhere. can we reuse the canonical one?`
- `i think there is a code-judo move here that makes this much simpler. can we reframe this so these branches disappear?`
- `this refactor moves complexity around, but does not really delete it. is there a way to make the model simpler?`

## Output expectations

Prioritize findings in this order:

1. Structural regressions.
2. Missed dramatic simplification.
3. Spaghetti and branching growth.
4. Boundary and type-contract problems.
5. File-size and decomposition concerns.
6. Modularity and abstraction issues.
7. Legibility and maintainability concerns.

Prefer fewer high-conviction comments over cosmetic noise.

## Approval bar

Do not approve only because behavior seems correct. Require:

- No structural regression.
- No obvious missed dramatic simplification.
- No unjustified file-size explosion.
- No obvious spaghetti growth.
- No hacky or magical abstraction that harms reasoning.
- No unnecessary wrapper, cast, or optionality churn.
- No architecture-boundary leak.
- No missed obvious decomposition.

Treat these as presumptive blockers:

- Incidental complexity remains despite a plausible simpler design.
- A file grows from under 1,000 lines to over 1,000 lines.
- Ad-hoc branching tangles an existing flow.
- Feature checks scatter across shared paths.
- An unnecessary abstraction or cast-heavy contract makes design indirect.
- A bespoke helper duplicates a canonical helper or uses the wrong layer.
