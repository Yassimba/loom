---
name: next
description: Claim the next actionable Beads task using bv and br. Use when the user asks what to work on next or asks to pick, grab, claim, or start the next task.
---

# Next

Select one ready task from the Beads dependency graph, claim it for this session, and start it. Run every `br` command through this skill's actor-aware launcher:

- macOS/Linux: `<this-skill-directory>/scripts/br-agent.sh`
- Windows: `& "<this-skill-directory>\scripts\br-agent.ps1"`

Replace `<this-skill-directory>` with the directory containing this `SKILL.md`. The launcher resolves one session-unique actor, passes it to `br --actor`, preserves all arguments and the exit code, and fails before `br` runs when it cannot identify the session.

1. Confirm the repository contains `.beads/` and both `br` and `bv` are on `PATH`. If any prerequisite is missing, report it and stop.
2. Run `bv --robot-next`. Use only `bv --robot-*` commands; bare `bv` opens an interactive TUI.
3. Accept only the returned top pick with a non-empty `claim_command`. Verify its current state with `<launcher> show <id> --json`: it must be open, ready, and unassigned. An assignment to another actor is a live claim.
4. Claim it atomically with `<launcher> update <id> --claim`. If the claim loses a race, rerun the selection from step 2; never force-reclaim another actor's task.
5. Run `<launcher> show <id> --json` again and read the full task. State the claimed ID and title, then treat its body as the current request and begin the work using the repository's normal workflow and relevant skills.

If `bv` returns no claimable task, report that the ready frontier is empty. Include blocked work only as context; do not claim it.

## Actor adapters

`BEADS_ACTOR` is the launcher's explicit, stable input and always wins. `CLAUDE_CODE_SESSION_ID`, `CODEX_SESSION_ID`, `CODEX_THREAD_ID`, `OPENCODE_SESSION_ID`, and `PI_SESSION_ID` are host adapters in that order. Loom's OpenCode plugin supplies `OPENCODE_SESSION_ID` from the documented `shell.env` session input; the other hosts do not provide a shared stable contract. OpenAI's [stable Codex environment-variable list](https://developers.openai.com/codex/config-file/environment-variables) does not include either Codex session identifier. Add new host adapters only when they identify one session uniquely. A human login name and a per-command process ID are not session identities.

Detected actors keep the complete raw session ID after a provider prefix. Strip only that prefix to resume the owning session:

| Beads assignee | Resume |
| --- | --- |
| `claude-<raw-id>` | `claude --resume <raw-id>` |
| `codex-<raw-id>` | `codex resume <raw-id>` |
| `opencode-<raw-id>` | `opencode --session <raw-id>` |
| `pi-<raw-id>` | `pi --session <raw-id>` |

Concurrent instances of one agent stay separate because each launcher process reads its own session ID. If the host does not expose a unique ID, stop and ask the user to set `BEADS_ACTOR=<provider>-<raw-id>` explicitly.

Pass an explicit override through `BEADS_ACTOR`; the launcher rejects `--actor` arguments so the real `br` process receives exactly one actor.

Done when exactly one ready task is claimed for the resolved actor and its work has begun, or when a named stop condition has been reported.
