# Product

<!-- impeccable:product-schema 1 -->

## Platform

adaptive

## Users

Loom serves curious developers who are new to coding-agent tooling, experienced agent users who want exact control, and non-technical teammates who need plain language and safe guidance.

## Product Purpose

Loom sets up and maintains coding agents, shared skills, pinned tools, Pi packages, project instructions, and searchable Wiki Vaults. Success means a user can choose outcomes they understand, review every resulting change, complete setup safely, and immediately use what was installed.

## Positioning

Loom provides one reviewed, reproducible setup surface across multiple coding agents while preserving user choices and existing local modifications.

## Operating Context

People run Loom in a terminal during first-time machine setup, when adding capabilities, when initializing repositories, and when updating or repairing an existing installation. Setup may involve global and project scope, multiple agents, external package managers, authentication, and long-running downloads or indexing.

## Capabilities and Constraints

- Setup starts from user goals and allows multiple goals at once.
- Users can inspect and change every selected capability; there is no default or Express path yet.
- Loom may automatically repair missing, outdated, or partially installed official resources when the repair is safe.
- Loom must ask before overwriting edits, replacing custom sources, changing authentication, or taking destructive action.
- Installation must preserve completed work, explain failures without exposing secrets, and support retry or resumption.
- The CLI supports macOS, Linux, native Windows, and WSL2.

## Brand Commitments

Use the Loom name and its direct, calm, plain-language voice. Keep expert detail available without requiring users to understand package managers, agent internals, or dependency terminology.

## Evidence on Hand

The repository contains the working Rust setup wizard, reviewed catalog metadata, install and update tests, Loom documentation, and the woven purple Loom logo. Do not invent adoption, performance, or reliability claims.

## Product Principles

- Ask what the person wants to accomplish before asking which tools they know.
- Explain every selection and machine change in plain language.
- Automate safe recovery while protecting intentional customization.
- Keep full control available without making expertise a prerequisite.
- Finish with verified, goal-specific actions rather than installation counts alone.

## Accessibility & Inclusion

The terminal experience must remain keyboard-first, mouse-usable, readable in narrow terminals, compatible with plain and dumb terminals, and understandable without prior coding-agent knowledge.
