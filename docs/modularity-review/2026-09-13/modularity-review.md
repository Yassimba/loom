# Modularity Review

**Scope**: `plugins/pi-guardrails`
**Date**: 2026-09-13

## Executive Summary

Pi Guardrails protects files, enforces workspace boundaries, and gates dangerous shell commands through three co-deployed Pi extension entrypoints. Its pure safety primitives and prompt event contract are modular, but the package needs attention in two high-volatility areas: the file-safety composition root and the configuration surface. The most important finding is that `extensions/path-access/index.ts` owns policy enforcement while importing workspace behavior from deep inside `/add-dir`, so three independently named features must share ordering and state knowledge to remain correct. Configuration changes have a similar cascade across TypeScript types, JSON Schema, merge rules, migrations, and a large string-addressed settings command.

## Coupling Overview

| Integration | [Strength](https://coupling.dev/posts/dimensions-of-coupling/integration-strength/) | [Distance](https://coupling.dev/posts/dimensions-of-coupling/distance/) | [Volatility](https://coupling.dev/posts/dimensions-of-coupling/volatility/) | [Balanced?](https://coupling.dev/posts/core-concepts/balance/) |
| ----------- | ----------------------------------------------------------------------------------- | ----------------------------------------------------------------------- | --------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Path Access runtime → Guardrails policies | [Functional](https://coupling.dev/posts/dimensions-of-coupling/integration-strength/) — shares rule compilation, tool applicability, severity ordering, and execution precedence | High — sibling feature boundary | High — safety behavior is expected to change | **No** |
| Path Access runtime → `/add-dir` workspace control | [Functional](https://coupling.dev/posts/dimensions-of-coupling/integration-strength/) — shares root state, scope vocabulary, trust rules, and persistence behavior | High — imports a deep command module | High — workspace behavior and Pi integration are expected to change | **No** |
| Bash target extraction → boundary enforcement | [Functional](https://coupling.dev/posts/dimensions-of-coupling/integration-strength/) — extractor embeds downstream eligibility decisions | High — shared/core layer to extension runtime | High — tool and path interpretation will evolve | **No** |
| Config types → schema, loader, migrations, and settings | [Model](https://coupling.dev/posts/dimensions-of-coupling/integration-strength/) plus functional duplication | High — five separate implementation surfaces | High — configuration and UI are expected to change | **No** |
| Settings command → feature-specific editors | [Functional](https://coupling.dev/posts/dimensions-of-coupling/integration-strength/) — conversions and mutations are repeated inside one command | Low — same command module and adjacent UI files | High — configuration and UI are expected to change | **No: low cohesion** |
| Permission Gate runtime → core rule engine | [Contract](https://coupling.dev/posts/dimensions-of-coupling/integration-strength/) — `Action`, `Rule`, and `Safety` discriminated contracts | High — extension to core boundary | High — safety behavior will evolve | **Yes** |
| Prompt producers → Herdr adapter | [Contract](https://coupling.dev/posts/dimensions-of-coupling/integration-strength/) — paired opened/closed events correlated by `prompt.id` | High — separate Pi entrypoint and optional external consumer | Moderate implementation volatility | **Yes** |
| Config loader → `@aliou/pi-utils-settings` | [Contract](https://coupling.dev/posts/dimensions-of-coupling/integration-strength/) — scoped loader API and explicit merge hook | High — package dependency | Low functional, moderate implementation volatility | **Yes** |

## Issue: File safety is composed inside the Path Access feature

**Integration**: Path Access runtime → Guardrails policies and `/add-dir` workspace control  
**Severity**: Critical

### Knowledge Leakage

`extensions/path-access/index.ts:16-21` imports policy compilation, blocked-tool sets, protection ranking, and rule construction from `extensions/guardrails/rules.ts`. The same runtime imports `WorkspaceRootControl` from the deep command path `extensions/guardrails/commands/add-dir` and knows its mutable `paths` set plus its `add(path, scope, ctx)` operation (`extensions/path-access/index.ts:15,101-105,156-166`). It also owns the implicit rule that protected-file policies execute before workspace-boundary checks (`extensions/path-access/index.ts:82-95`).

This is [functional coupling](https://coupling.dev/posts/dimensions-of-coupling/integration-strength/): the modules share safety requirements and execution semantics rather than a narrow integration contract. The dependency graph is technically acyclic, but ownership is circular—Guardrails registers Path Access, while Path Access knows Guardrails policy and command internals.

### Complexity Impact

A developer changing file-policy precedence, workspace scopes, project trust, or approval persistence must reason across the bootstrap, Path Access runtime, policy rules, `/add-dir`, grants, configuration, and prompt result mapping. That is more than the 4±1 units a developer can reliably hold at once, making outcomes [complex and less predictable](https://coupling.dev/posts/core-concepts/complexity/). A locally reasonable refactor—such as registering policies as a separate `tool_call` hook—can silently change which rule wins because Pi hook ordering becomes part of the safety contract.

### Cascading Changes

- Adding a workspace scope changes `/add-dir`, Path Access prompt results, trust filtering, config persistence, and runtime grant mapping.
- Changing policy precedence changes `checkPolicyTargets`, Path Access blocking behavior, and user-visible reasons.
- Moving Path Access to an independent entrypoint would require reproducing Guardrails startup ordering and workspace state.
- Replacing `/add-dir` UI would still force changes in Path Access because its runtime imports the command-owned control type.

The [distance](https://coupling.dev/posts/dimensions-of-coupling/distance/) is high at the package-module level, and the user confirmed all three feature areas are highly [volatile](https://coupling.dev/posts/dimensions-of-coupling/volatility/). High strength plus high distance plus high volatility violates the [balance rule](https://coupling.dev/posts/core-concepts/balance/).

### Recommended Improvement

Reduce distance rather than pretending these requirements are independent. Put protected-file policy and workspace-boundary ordering in one feature-neutral file-safety composition module owned by the Guardrails entrypoint. Move workspace-root state and mutation out of the `/add-dir` command into a small workspace module; `/add-dir` and reactive approvals should both call it.

Keep prompts, command registration, and persistence adapters separate, but make them depend inward on the workspace and file-safety modules. The trade-off is one explicit internal composition module, but it matches the already strong cohesion and removes deep feature-to-command imports. Do not split these checks into separately loaded hooks unless their precedence becomes an explicit Pi-level contract.

## Issue: Configuration has several manually synchronized sources of truth

**Integration**: `GuardrailsConfig` → JSON Schema, defaults, merge rules, migrations, and settings UI  
**Severity**: Critical

### Knowledge Leakage

The configuration model in `src/shared/config/types.ts:22-161` is repeated manually in `schema.json`, interpreted again by merge functions in `src/shared/config/loader.ts:21-96`, represented through string IDs in `extensions/guardrails/commands/settings/index.ts:262-569`, and transformed by eleven ordered migrations. Defaults add another functional representation in `src/shared/config/defaults.ts`.

This starts as [model coupling](https://coupling.dev/posts/dimensions-of-coupling/integration-strength/)—the same configuration concepts are shared—but becomes stronger functional coupling because each surface independently encodes validation, precedence, defaults, and mutation behavior. For example, allowed paths are typed in core, copied into JSON Schema, deduplicated in the loader, filtered in settings, and normalized again by `PathListEditor`.

### Complexity Impact

A field addition can compile successfully while leaving editor validation, persisted schema, merge semantics, or UI mutation stale. String paths such as `features.*`, `pathAccess.allowedPaths`, and `permissionGate.*` are not checked against `GuardrailsConfig`, so renames do not produce complete compiler errors. Developers must inspect at least types, schema, defaults, loader, migration history, settings sections, and tests before predicting the effect of one model change.

### Cascading Changes

- Changing `WorkspaceRoot` requires synchronized edits to types, schema, merge identity, settings exposure, `/add-dir`, and migration compatibility.
- Changing `PatternConfig` affects policy rules, dangerous patterns, schema definitions, editors, examples, and persistence conversions.
- Changing array merge semantics affects all three scopes and any UI that assumes replacement rather than additive behavior.
- Renaming a settings field can leave string-addressed reads and writes silently disconnected.

Because configuration and UI are planned to change substantially, this high-strength, high-[distance](https://coupling.dev/posts/dimensions-of-coupling/distance/) relationship is not neutralized by low volatility. It is an active [modularity](https://coupling.dev/posts/core-concepts/modularity/) risk.

### Recommended Improvement

Generate `schema.json` from the TypeScript configuration types using the schema tooling already supported by `@aliou/pi-utils-settings`; keep migrations as the explicit compatibility boundary. Then centralize each field's normalization and merge identity beside its model—for example, one `AllowedPath` normalizer/key and one `WorkspaceRoot` merge function reused by loader and UI.

Do not create a generic configuration framework. Keep `ConfigLoader`, but replace untyped field strings with a bounded union or feature-local constants generated from the actual settings sections. The cost is a schema-generation check and a few explicit adapters; the benefit is that model changes fail in one deterministic place rather than drifting across five surfaces.

## Issue: Target extraction contains a downstream authorization decision

**Integration**: `src/shared/paths/bash-paths.ts` → Path Access boundary checks  
**Severity**: Significant

### Knowledge Leakage

`BashPathTarget` exposes `checkPathAccess` (`src/shared/paths/bash-paths.ts:23-26`), and extraction computes it using the current workspace boundary and filesystem plausibility (`src/shared/paths/bash-paths.ts:70-91`). Its comments explicitly justify behavior using the downstream fact that `checkPathAccess` allows in-workspace paths. The Path Access runtime then treats the boolean as authorization-pipeline eligibility (`extensions/path-access/index.ts:107-109`), while policy checks intentionally inspect every target regardless of that flag (`extensions/path-access/index.ts:42-51`).

The extractor therefore shares functional knowledge of two consumers: policies require broad evidence, while workspace enforcement requires prompt-suppression heuristics. This is implicit [functional coupling](https://coupling.dev/posts/dimensions-of-coupling/integration-strength/) disguised as a generic shared target model.

### Complexity Impact

Changes to command classification or plausibility can either create noisy prompts or remove policy evidence. A developer cannot determine whether changing `maybePathLike`, creation-command handling, or missing-path suppression is safe by reading the extractor alone; they must trace both policy and boundary consumers. The boolean name also hides why a candidate was excluded, so test failures expose outcomes without preserving the evidence behind them.

### Cascading Changes

- Supporting a new tool or shell wrapper changes command classification, extraction, policy matching, and prompt behavior.
- Changing missing-path heuristics can alter both authorization prompts and protected-file enforcement.
- Moving workspace boundaries from `cwd` to multiple roots changes assumptions embedded in a nominally shared parser.
- Adding a third target consumer must either inherit Path Access semantics or reinterpret the boolean.

The safety domain is highly volatile, so this high-strength, cross-layer relationship is unbalanced even though both modules ship together.

### Recommended Improvement

Reduce [integration strength](https://coupling.dev/posts/dimensions-of-coupling/integration-strength/) with a factual target contract. Replace `checkPathAccess` with neutral evidence such as `explicitPath` or `plausibleLocalPath`; keep canonical path and unresolved-expansion state. Apply workspace membership and prompt-suppression policy in the file-safety composition module, not in shell extraction.

Keep AST walking, command argument classification, and plausibility tables together—they are strongly coupled and appropriately close. The trade-off is a slightly richer target value, but it prevents parsing code from owning authorization semantics and makes each consumer's decision testable.

## Issue: Settings combines unrelated feature forms and repeats adapters

**Integration**: Settings command → policy, path-access, and permission-gate configuration  
**Severity**: Significant

### Knowledge Leakage

`extensions/guardrails/commands/settings/index.ts` contains feature metadata, a 190-line policy editor, the config-store adapter, draft mutation helpers, pattern/path conversions, all feature sections, and change dispatch. Policy pattern conversion appears twice in `createPolicyRuleEditor` (`settings/index.ts:132-201`), while `patternSubmenu` and `patternConfigSubmenu` implement two more representations (`settings/index.ts:306-385`). Policy ID collision behavior is separately implemented by `addPolicyRuleDraft` (`settings/utils.ts:72-101`) and `appendPolicyRule` (`settings/examples.ts:333-358`).

This is both duplicated functional knowledge and [low cohesion](https://coupling.dev/posts/core-concepts/balance/): policy editing, path editing, permission editing, and generic persistence sit at low distance despite having largely separate change reasons.

### Complexity Impact

A developer changing one feature must navigate a 500-plus-line command that also owns the other two. Similar-looking adapters have different semantics: policy pattern editing discards descriptions, while permission allow/deny editing preserves nonredundant descriptions. The duplication makes it unclear whether that difference is intentional or drift.

### Cascading Changes

- Adding a pattern field requires updating several editor conversions and the add-rule wizard.
- Changing policy ID rules requires updating normal rule creation and example insertion.
- Adding a Path Access field expands the same section builder and string-based dispatcher used by unrelated features.
- Changing editor behavior risks policy and permission forms because they share `PatternEditor` through different adapters.

Low strength plus low distance is another unbalanced state under the [balance rule](https://coupling.dev/posts/core-concepts/balance/): unrelated responsibilities accumulate cognitive load even without direct runtime coupling.

### Recommended Improvement

Keep one `/guardrails:settings` command and one config store, but extract three feature-owned section builders: policies, Path Access, and Permission Gate. Add two pure pattern adapters—optional-description and required-description—and one shared policy-ID collision helper. Return ordinary `SettingsSection` values; no registry, plugin framework, or dependency injection layer is needed.

The trade-off is several smaller files, but each change gains a clear location and the existing settings library remains the stable [contract](https://coupling.dev/posts/dimensions-of-coupling/integration-strength/). Extract only the repeated behavior and feature sections; leave one-off orchestration in the command.

---

_This analysis was performed using the [Balanced Coupling](https://coupling.dev) model by [Vlad Khononov](https://vladikk.com)._
