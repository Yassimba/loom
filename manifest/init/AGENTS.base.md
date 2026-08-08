# AGENTS.md

## Conversational Style

- Technical prose: short, direct, kind ("Thanks @user", not "Thanks so much @user!"). No filler, no agreement theater, no emojis in commits, issues, MR comments, or code.
- Never add any attributions to yourself in commits
- NEVER commit unless user asks
- Write in ASD-STE100 Simplified Technical English — everywhere, especially comments/docstrings

  **Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:

- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:

- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:

- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:

- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:

```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.

## Code Quality

- Before writing significant amounts of new code, look for existing utilities or mechanisms that could solve the problem. Avoid expanding the task to unrelated issues, but do not confuse keeping the task focused with minimizing the size of the implementation. Prefer addressing the underlying architectural problem over adding a localized workaround, even when doing so requires a substantial refactor or rearchitecture. Ask the user for guidance if in doubt about whether to attempt a larger refactor or not. !! This one is important!
- Don't use comments to narrate code, but do use them to explain invariants and why something unusual was done a particular way. Make sure that a comment will make sense to somebody who's reading the code for the first time. Prefer plain language, avoid jargon, and don't be afraid to be more verbose if it's necessary to explain something well. Giving examples for exampl for the kind of (Python) code we're trying to model at this particular point can be useful to future readers Always use ASD-STE100 Simplified Technical English
- All significant changes must be tested. Add or update focused tests for semantic changes when existing coverage does not already establish the intended behavior.
- Look to see if your tests could go in an existing file before adding a new file for your tests.
- Every new test names its capability and seam (decision table, workflow scenario, or external contract), states which production fault would fail it, and — for parity/migration tests — an expiry. One authoritative owner per behavior, at most one high-seam sentinel.
- Check the actual implementation of packages for external API type definitions instead of guessing
- Do not preserve backward compatibility unless the user explicitly asks for it
- Prefer clean and SOLID code instead of an "easy migration" when implementing always look at python-patterns if there is a common pattern you can use if so steal it and the structure
- Always use Guard clauses if possible.
- If dependency types are outdated, upgrade the dependency instead of weakening our code.

## Iron Law: Fix ALL Problems

- When you find a bug during testing, fix it. No exceptions.
- Never dismiss a problem as "pre-existing", "not related", or "out of scope".
- Never assume an external tool has the bug. The bug is in our code until proven otherwise with evidence.

## Commands

After code changes, run the full quality gate and inspect complete output:

Rules:

- Fix every error, warning, and info before claiming completion.
- Do not use truncated output or `tail` for verification.
- Do not run broad or expensive commands unless the user asks.
- If you create or modify a test file, you MUST run that test file and iterate until it passes.

## Changelog

Location: `CHANGELOG.md`

### Format

Use these sections under `## [Unreleased]`:

- `### Breaking Changes` - API changes requiring migration
- `### Added` - New features
- `### Changed` - Changes to existing functionality
- `### Fixed` - Bug fixes
- `### Removed` - Removed features

### Rules

- Before adding entries, read the full `[Unreleased]` section to see which subsections already exist
- New entries ALWAYS go under `## [Unreleased]` section
- Append to existing subsections (e.g., `### Fixed`), do not create duplicates
- NEVER modify already-released version sections (e.g., `## [0.12.2]`)
- Each version section is immutable once released

### Forbidden Git Operations

These commands can destroy other agents' work:

- `git reset --hard` - destroys uncommitted changes
- `git checkout .` - destroys uncommitted changes
- `git clean -fd` - deletes untracked files
- `git stash` - stashes ALL changes including other agents' work
- `git add -A` / `git add .` - stages other agents' uncommitted work
- `git commit --no-verify` - bypasses required checks and is never allowed

## Agent skills

### Domain docs

Single-context: `CONTEXT.md` + `ai-docs/adr/` at the repo root. See `ai-docs/agents/domain.md`.

### Editor

Zed — deep links use `zed://file/{path}:{line}`.
