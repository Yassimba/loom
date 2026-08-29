# Autoresearch: explain-code-flow speed and token efficiency

## Objective
Reduce the end-to-end agent latency and total model tokens required by `skills/explain-code-flow` while preserving or improving evidence accuracy, figure choice, diagram quality, and walkthrough usefulness.

Optimize the reusable workflow, not named benchmark cases. Prefer progressive disclosure, compact handoff contracts, deterministic scripts, fewer rereads, and safe parallelism. Do not remove work merely to improve the metric.

## Metrics
- **Primary:** `prompt_words` (estimated repeated agent-context words, lower is better). This models the main skill load plus evidence-worker and four parallel drawing-worker payloads.
- **Secondary:** `skill_words`, `evidence_words`, `drawing_worker_words`, `check_ms`, `quality_checks`.
- **Milestone validation:** real fresh subagent runs report wall time and full token usage. Static prompt words are a screening proxy, never final proof.

## How to Run
`./.auto/measure.sh` emits `METRIC name=value` lines.

Then run `./.auto/checks.sh`. A candidate is invalid if any check fails.

## Public Development Workloads
Use varied feature shapes when a real agent run is available:
1. Rust, broad/concurrent: Loom install-plan execution from CLI dispatch through plan construction, execution lanes, verification, and report.
2. TypeScript, compact/event-driven: OpenAI Fast Mode session selection, request rewrite, display, persistence, and shutdown.

Alternate order and paraphrase prompts. Stop after validated `walkthrough.html`; do not launch interactive Plannotator during measurement.

## Files in Scope
- `skills/explain-code-flow/SKILL.md` — orchestration and progressive disclosure.
- `skills/explain-code-flow/references/*.md` — worker contracts and conditional detail.
- `skills/explain-code-flow/scripts/*` — deterministic generation and validation.
- Tests for those scripts.

## Off Limits
- `.auto/**` benchmark, checks, logs, and scoring code.
- Production Loom code outside `skills/explain-code-flow/**` and focused tests.
- Model, thinking level, tool access, prompts, benchmark ordering, or evaluator configuration.
- Hidden workloads, judge data, previous outputs, or environment-based benchmark detection.

## Quality Floor
Every kept change must preserve:
- verified production entry-to-final-effect evidence and honest composition gaps;
- source-backed nodes and edges with current anchors;
- justified Overview and Spine plus only qualifying nonredundant figures;
- valid `.py`, `.html`, `.svg`, PNG, Markdown, and inlined HTML artifacts;
- readable diagrams, exact identifiers, value-in/value-out spine, terminal outcomes;
- concise anchored prose and successful deterministic checks.

Static checks do not prove semantic quality. At milestones, freeze the candidate and use blinded reviewers on public and held-out feature shapes. A quality regression invalidates token or speed gains.

## Anti-cheating
- Never mention benchmark feature names, benchmark paths, expected anchors, prompts, or scoring implementation in the skill.
- Never special-case repository paths, environment variables, task wording, or evaluator state.
- Never reduce required figures, anchors, prose, checks, model effort, or tool access solely to lower metrics.
- Never modify measurement or checks.
- Prefer general improvements that explain why they should transfer to unseen languages and feature shapes.

## What's Been Tried
- Baseline pending.
