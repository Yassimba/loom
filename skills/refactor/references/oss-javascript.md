# JavaScript and TypeScript library candidates

Apply the selection rules in the [library catalog](oss-libraries.md). Verify each candidate against the project's deployed runtimes and required semantics.

## Web platform

- **EventTarget** — local subscriber registries when event and listener semantics fit.
- **Intl.NumberFormat** — currency separators, rounding, grouping, and symbols.
- **HTMLDialogElement** — modal backdrops, Escape handling, inert backgrounds, and focus management; verify accessibility in the target browsers.
- **CSS Grid/Flex, container/viewport units, and aspect-ratio** — resize listeners and viewport arithmetic used only for layout.
- **Object.groupBy** — reduce loops that group records by a derived key, when the deployed runtime supports it.

## General

- **vue** (`Transition` and local components) — custom transition systems and repeated presentation markup in Vue applications.
- **tailwindcss** — repeated utility CSS when the project already uses Tailwind; account for markup and build migration cost otherwise.
- **@microsoft/fetch-event-source** — EventSource connection, retry, abort, response validation, and event-stream parsing machinery when its semantics fit.
- **xstate** — distributed workflow statuses, transition checks, delayed callbacks, and effect lifecycle management when a state-machine model earns its cost.
- **openapi-typescript** — hand-copied OpenAPI request/response vocabulary and TypeScript declarations.
- **openapi-fetch** — endpoint strings, parameter placement, request serialization, and response typing over an authoritative OpenAPI contract.
- **@vueuse/core** — Vue component-owned timers, animation frames, media queries, keyboard chords, listeners, and WebSocket lifecycle helpers.
- **@tanstack/vue-query** — remote loading/error state, polling, request deduplication, and cache invalidation in Vue.
- **pinia-plugin-persistedstate** — localStorage synchronization and Pinia hydration plumbing; check persistence and migration semantics.
- **lucide-vue-next** — duplicated inline SVG icon inventories in Vue when the icon set meets product needs.
