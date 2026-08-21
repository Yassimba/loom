---
name: setup-project
description: Configure this repo for the engineering skills — issue tracker (Beads, GitHub, GitLab, or local markdown), domain-doc layout, editor deep links, ai-docs gitignore. Use when the user wants a repo set up for the engineering skills, after a fresh `loom init`, or when another skill finds `ai-docs/agents/` missing.
---

# Setup Project

Scaffold the per-repo configuration that the engineering skills assume:

- **Issue tracker** — where issues live (GitHub by default; local markdown is also supported out of the box)
- **Domain docs** — where `CONTEXT.md` and ADRs live, and the consumer rules for reading them
- **Editor** — which editor deep links open in (skills that emit clickable `file:line` links)

## Process

### 1. Explore

Look at the current repo to understand its starting state. Read what's actually there:

- `git remote -v` and `.git/config` — is this a GitHub repo? Which one?
- `.beads/` at the repo root, and whether the `br` CLI is on PATH — an installed Beads setup is the strongest tracker signal
- `AGENTS.md` and `CLAUDE.md` at the repo root — does either exist? Is there already an `## Agent skills` section in either?
- `CONTEXT.md` and `CONTEXT-MAP.md` at the repo root
- `ai-docs/adr/` and any `src/*/ai-docs/adr/` directories
- `ai-docs/agents/` — does this skill's prior output already exist?
- `.gitignore` — is `ai-docs/` already ignored?
- `ai-docs/plans/` — sign that a local-markdown issue tracker convention is already in use
- An `### Editor` entry in the existing `## Agent skills` block — is the editor already configured?

Done when: every item above has an observed answer — a guess is a missing observation.

### 2. Present findings and ask

Summarise what's present and what's missing. Then walk the user through the two decisions **one at a time** — present a section, get the user's answer (AskUserQuestion fits the choice lists well), then move to the next.

Assume the user does not know what these terms mean. Each section starts with a short explainer (what it is, why these skills need it, what changes if they pick differently). Then show the choices and the default.

**Section A — Issue tracker.**

> Explainer: The "issue tracker" is where issues live for this repo. Skills like `to-tickets` and `to-spec` read from and write to it — they need to know whether to call `gh issue create`, write a markdown file under `ai-docs/plans/`, or follow some other workflow you describe. Pick the place you actually track work for this repo.

Propose **Beads** first when `.beads/` exists or the `br` CLI is installed — a machine set up for Beads almost always wants its repos tracked there. Otherwise propose the tracker the `git remote` points at — GitHub, or GitLab (`gitlab.com` or self-hosted). The full menu:

- **Beads** — issues live in `.beads/issues.jsonl` in this repo, managed with the `br` CLI, triaged with `bv` (dependency-aware, built for agents)
- **GitHub** — issues live in the repo's GitHub Issues (uses the `gh` CLI)
- **GitLab** — issues live in the repo's GitLab Issues (uses the [`glab`](https://gitlab.com/gitlab-org/cli) CLI)
- **Local markdown** — issues live as files under `ai-docs/plans/<feature>/` in this repo (good for solo projects or repos without a remote)
- **Other** (Jira, Linear, etc.) — ask the user to describe the workflow in one paragraph; the skill will record it as freeform prose

**Section B — Domain docs.**

> Explainer: Some skills (`improve-codebase-architecture`, `diagnosing-bugs`, `tdd`) read a `CONTEXT.md` file to learn the project's domain language, and `ai-docs/adr/` for past architectural decisions. They need to know whether the repo has one global context or multiple (e.g. a monorepo with separate frontend/backend contexts) so they look in the right place.

Confirm the layout:

- **Single-context** — one `CONTEXT.md` + `ai-docs/adr/` at the repo root. Most repos are this.
- **Multi-context** — `CONTEXT-MAP.md` at the root pointing to per-context `CONTEXT.md` files (typically a monorepo).

**Section C — Editor.**

> Explainer: Some skills (`show-me`, and any that render clickable source links) emit deep links that open a file at a line in your editor. They need to know which URL scheme your editor answers to. Pick the editor you actually read code in.

- **VS Code** — `vscode://file/{path}:{line}`
- **Zed** — `zed://file/{path}:{line}`
- **Cursor** — `cursor://file/{path}:{line}`
- **JetBrains** (IntelliJ/PyCharm via Toolbox) — `idea://open?file={path}&line={line}`
- **None** — skip deep links; skills render plain `path:line` text instead

For anything else, ask for the editor's URL template with `{path}` and `{line}` placeholders and record it verbatim. `{path}` is always absolute.

Done when: the user has answered all three sections — a defaulted answer the user never saw is not an answer.

### 3. Confirm and edit

Show the user a draft of:

- The `## Agent skills` block to add to whichever of `CLAUDE.md` / `AGENTS.md` is being edited (see step 4 for selection rules)
- The contents of `ai-docs/agents/issue-tracker.md` and `ai-docs/agents/domain.md`

Let them edit before writing.

Done when: the user has approved the draft, edited or as-is.

### 4. Write

**Pick the file to edit:**

- If `CLAUDE.md` exists and is only an `@AGENTS.md` pointer (the `loom init` pattern), edit `AGENTS.md`.
- Else if `CLAUDE.md` exists, edit it.
- Else if `AGENTS.md` exists, edit it.
- If neither exists, ask the user which one to create — don't pick for them.

If an `## Agent skills` block already exists in the chosen file, update its contents in-place, leaving the surrounding sections exactly as the user wrote them. In a `loom init`-managed file, place the block **outside** the `ai-setup:section` fences (typically at the end) — text inside the fences belongs to the templates and refreshes on sync.

The block:

```markdown
## Agent skills

### Issue tracker

[one-line summary of where issues are tracked]. See `ai-docs/agents/issue-tracker.md`.

### Domain docs

[one-line summary of layout — "single-context" or "multi-context"]. See `ai-docs/agents/domain.md`.

### Editor

[editor name] — deep links use `[template with {path} and {line}]`.
```

Then write the two docs files using the seed templates in this skill folder as a starting point:

- [issue-tracker-beads.md](./issue-tracker-beads.md) — Beads issue tracker
- [issue-tracker-github.md](./issue-tracker-github.md) — GitHub issue tracker
- [issue-tracker-gitlab.md](./issue-tracker-gitlab.md) — GitLab issue tracker
- [issue-tracker-local.md](./issue-tracker-local.md) — local-markdown issue tracker
- [domain.md](./domain.md) — domain doc consumer rules + layout

For "other" issue trackers, write `ai-docs/agents/issue-tracker.md` from scratch using the user's description.

Add `ai-docs/` to `.gitignore` (create the file if needed; skip if already covered) — `ai-docs/` is local agent working space: plans, briefs, brainstorm sessions, and these config docs stay out of the repo's history.

Done when: the `## Agent skills` block names all three choices, `ai-docs/agents/issue-tracker.md` and `ai-docs/agents/domain.md` both exist, and `.gitignore` covers `ai-docs/`.

### 5. Done

Tell the user the setup is complete and which engineering skills will now read from these files. Mention they can edit `ai-docs/agents/*.md` directly later — re-running this skill is only necessary if they want to switch issue trackers or restart from scratch.
