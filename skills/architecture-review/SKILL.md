---
name: architecture-review
description: Review an existing codebase's architecture with parallel subagents, combining Balanced Coupling diagnosis with deep-module improvement candidates. Use for architecture audits, modularity reviews, coupling analysis, deepening opportunities, or when changes are unexpectedly expensive.
license: CC-BY-NC-SA-4.0
---

# Architecture Review

Run one evidence-led architecture workflow. Diagnose whether integrations are balanced, then identify where deeper modules would concentrate complexity. The review explains **why** change is expensive before proposing **where** complexity should move.

## Non-negotiable rules

- Read actual calls, imports, shared models, queries, events, tests, and failure paths. Directory structure alone is not evidence.
- Establish the abstraction level before judging distance.
- Assess integration strength, distance, and volatility together. Commit frequency locates hot spots; it does not prove business volatility.
- Rank at most five issues that are both unbalanced and plausibly volatile. Include one healthy or intentional integration.
- Prefer balancing coupling over reducing it. Strong coupling belongs at low distance; high distance requires a narrow contract.
- Apply the deletion test to proposed modules. A useful module concentrates complexity when deleted; a shallow pass-through merely moves it.
- The interface is the test surface. Interface means every fact a caller must know: operations, invariants, ordering, errors, configuration, and performance.
- During review, recommend direction only. Do not design methods, parameters, DTO fields, or packages before the user selects a candidate.
- Subagents inspect and challenge. The parent owns questions, adjudication, ranking, and files.
- If parallel read-only subagents cannot run, stop and report the capability failure instead of silently substituting a shallow single-agent scan.

## Working vocabulary

- **Module** — an interface plus its implementation, at any scale.
- **Interface** — everything callers must know to use the module correctly.
- **Depth** — leverage delivered per unit of interface knowledge. A deep module hides substantial implementation; a shallow module does not.
- **Seam** — where behavior can vary without editing the caller.
- **Adapter** — an implementation that satisfies an interface at a seam.
- **Leverage** — capability gained by callers from one interface.
- **Locality** — change, knowledge, bugs, and verification concentrated in one place.

Balanced Coupling operational model:

- Strength: intrusive → functional → model → contract.
- Distance: code separation plus team ownership, deployment lifecycle, and runtime synchronization.
- Volatility: probability of essential business change, with implementation/provider volatility considered separately.
- `BALANCE = (STRENGTH XOR DISTANCE) OR NOT VOLATILITY`.
- High strength + high distance + high volatility is urgent. Low strength + low distance suggests low cohesion.

Read `references/coupling-model.md` before scoring. Read `references/deep-modules.md` before shaping improvement candidates. Read `references/report-contract.md` and the complete `assets/template.html` before writing artifacts.

## Phase 1 — Frame

1. Use the user's scope. If none is given, ask once: entire codebase, directory, or named modules.
2. Read project instructions, domain glossary or context map, relevant requirements, and relevant ADRs.
3. Use meaningful commit history only to focus an unspecified scan. Widen when no hot spot exists.
4. Read enough source to identify responsibilities, runtime flows, callers, tests, and external integrations.
5. Present a compact context summary: modules, integrations, domain classification, and assumptions about ownership/deployment. Ask the user to validate it.
6. Ask one question at a time only when the answer can change severity, rank, or recommendation. Prefer multiple choice. Resolve essential volatility, implementation volatility, team ownership, deployment, and sync/async runtime facts.

**Complete when:** scope, abstraction level, relevant domain language, volatility, ownership, and deployment assumptions are explicit.

## Phase 2 — Fork

Discover executable read-only subagents first. Launch exactly one top-level workflow. Inside it:

1. Run three analysts in parallel.
2. Pass their ordered outputs to one adversarial critic.
3. Return all four outputs to the parent.

Use role partitioning, not arbitrary directory partitioning:

| Role | Mission |
| --- | --- |
| Domain mapper | Map module responsibilities, domain language, core/supporting/generic classification, runtime flows, ownership/deployment evidence, and coverage gaps. |
| Integration analyst | Record concrete integrations, shared knowledge, implicitness, strength, all forms of distance, volatility, balance, and cascading-change scenarios. |
| Deep-module analyst | Find shallow interfaces, leakage across seams, weak locality, deletion-test outcomes, dependency categories, and test-surface problems. |
| Adversarial critic | Read the three outputs; challenge evidence, abstraction levels, volatility claims, speculative seams, ADR conflicts, and simpler do-nothing/co-location options. |

Every child prompt must include the scope, abstraction level, domain vocabulary, operational coupling model, deep-module vocabulary, and this evidence record:

```text
ID | observed/inferred | A -> B | abstraction level | path:line evidence |
shared knowledge | implicit/explicit | strength + rationale |
technical/team/deploy/runtime distance | essential/implementation volatility + source |
change vector | cascading edits | depth/seam observation |
counterevidence | confidence | inspected/uninspected paths
```

Child contract:

- Read-only: no edits, writes, branches, or user questions.
- Facts and inferences are labeled separately.
- Every concern has at least two source citations; one may cite a shared rule or schema.
- Each analyst returns at most five records plus coverage limits.
- Analysts do not prescribe exact replacement interfaces.
- The critic cites record IDs and source when accepting or rejecting claims.

**Complete when:** the parent has three independent evidence sets and one adversarial response, or has reported a subagent infrastructure blocker.

## Phase 3 — Join and diagnose

Build one evidence ledger. Verify disputed or load-bearing claims directly against source. Never strengthen a child claim without adding evidence.

For every candidate integration:

1. Name the shared knowledge and whether it is implicit.
2. Classify strength: intrusive, functional, model, or contract.
3. Record technical, organizational, deployment, and runtime distance.
4. Separate essential business volatility from implementation volatility.
5. Apply the balance rule at the declared abstraction level.
6. Show a plausible change vector and concrete cascading edits.
7. Apply the pain counterfactual: explain why doing nothing costs more than the recommendation.

Unknown ownership or volatility is `Needs context`, not Critical. A finding with fewer than two citations is `Unverified`, cannot exceed Speculative confidence, and cannot be the top recommendation. Merge overlapping coupling and depth observations into one issue.

For remediation, choose one direction before discussing module shape:

- reduce strength with a narrower contract;
- reduce distance by co-locating strongly coupled behavior;
- deepen a module so complexity and tests concentrate behind one interface;
- accept and monitor because volatility is low or change cost exceeds benefit.

Use dependency categories and seam rules from `references/deep-modules.md`. One adapter is a hypothetical seam; two justified adapters make it real.

**Complete when:** no more than five ranked findings pass the evidence, context, balance, deletion, and counterfactual gates; verified non-actions are separated as accepted/monitor items.

## Phase 4 — Report

Markdown is canonical. Write:

- `docs/architecture-review/<YYYY-MM-DD>/architecture-review.md`
- `docs/architecture-review/<YYYY-MM-DD>/architecture-review.html`

Honor a user-supplied output directory. Generate HTML from the canonical content using `assets/template.html`; do not author the two reports independently.

Each issue includes evidence, coupling dimensions, knowledge leakage, complexity impact, cascading changes, deep-module diagnosis, remediation direction, trade-off, severity, confidence, and a before/after visual. Severity describes current risk; confidence describes certainty in the recommendation. End with one ranked top recommendation.

Verify matching issue IDs, order, titles, severity, confidence, evidence, recommendations, links, scope, date, and attribution in both files. Confirm template placeholders are gone and HTML remains readable without network access.

The report footer must be exactly:

```markdown
---

_This analysis was performed using the [Balanced Coupling](https://coupling.dev) model by [Vlad Khononov](https://vladikk.com)._
```

**Complete when:** both artifacts exist, parity checks pass, and the user receives their paths plus the top recommendation.

## Phase 5 — Selected candidate

After the user chooses one finding, interview the decision frontier one question at a time. Find environmental facts yourself; ask the user only for decisions. Settle constraints, what belongs behind the seam, dependency categories, test survival, and the evidence for a real seam. Do not edit production code during this phase.

If the user asks to compare exact interfaces, read `references/design-it-twice.md` and run its parallel design workflow. Otherwise stop with an agreed direction and explicit open decisions.
