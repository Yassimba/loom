# Library catalog

A seed, not a census — examples that prime the hunt, grown by every scan (the growth step lives in `SKILL.md`). Each entry: the package and the hand-roll it retires. Group by ecosystem; stdlib counts as a package with zero install cost. Entries may be pruned when a library dies or a better replacement takes its slot.

## Standing orders

Apply these wherever this catalog is in hand:

- prefer, in order: stdlib → an existing dependency → a new package that deletes real code; add packages through the project's dependency tool (`uv add`, `npm install`, `cargo add`, …), never by editing the manifest
- flag underused existing dependencies that could absorb custom code
- diff hand-typed copies of upstream vocabularies, enums, or schemas against their source — drift bugs hide there

## Python — stdlib

- **graphlib.TopologicalSorter** — hand-rolled topological sorts and load-order loops; `CycleError` names the actual cycle.
- **functools.lru_cache / cache** — single-site memoisation dicts.
- **pathlib.Path.from_uri** (3.13+) — `file://` URI parsing incl. Windows drive-letter and UNC edge cases.
- **difflib.get_close_matches** — basic "did you mean" suggestions.
- **textwrap.wrap** — word-boundary wrapping, maximum line counts, and long-word/hyphen policies.

## Python — general

- **cachetools** — bounded LRU/TTL caches: OrderedDict+lock eviction loops, FIFO-masquerading-as-LRU dicts.
- **stamina** — retry loops, especially ones missing backoff; typed, opinionated wrapper over tenacity — prefer it.
- **msgspec** — hand-rolled JSON/msgpack encode-decode and validation loops; the fast alternative when pydantic's weight is not needed.
- **dacite** — hand-written `from_dict` factories that build nested dataclasses from mappings.
- **autoregistry** — manual registry dicts plus registration decorators keyed on class or function names.
- **lazy-loader** — PEP 562 lazy re-export tables written three times (TYPE_CHECKING mirror, exports dict, `__all__`).
- **typing_inspection** (ships with pydantic) — PEP 695 alias unwrapping, union-arm extraction, Literal/Optional introspection.
- **referencing** + **jsonschema** — JSON Schema `$ref`/`$defs`/anchor resolution and keyword semantics (`if/then/else`, `const`, `patternProperties`).
- **pydantic** (when already a dep) — `__post_init__` validators, hand-written discriminated unions, hand-emitted JSON Schema fragments, repr-based fingerprinting (`model_dump_json` + hash).
- **annotated-types** — hand-rolled Ge/Gt/Le/Lt/Interval threshold vocabularies.
- **fast-depends / lagom / svcs** — hand-rolled DI containers: type-keyed registries, signature introspection, cycle detection.
- **rapidfuzz** — fuzzy matching beyond difflib's quality/speed.
- **boltons** (fileutils) — atomic write (temp + fsync + replace + mode).
- **xattr** — macOS/Linux extended-attribute and ACL copying; retires raw ctypes `copyfile` bindings.
- **yoyo-migrations** — SQL migration ledgers: version tables, checksums, apply-pending.
- **deepdiff / DeepHash** — structural diffing and content-hashing of nested objects.
- **intervaltree / sortedcontainers** — linear scans over span/interval collections.
- **more-itertools** — groupby-and-freeze idioms, windowing, bucketing.
- **stevedore** — entry-point plugin loading with failure isolation.
- **blinker / pluggy** — observer/event dispatch and hook systems (fit only when the authoring surface can change).
- **ruamel.yaml** — YAML round-tripping with anchors, merge keys, comments, line/col info; also its resolver for scalar-schema regex tables.
- **httpx** (+ httpx-auth) — hand-rolled HTTP transports; one API for sync+async.
- **fastapi** + `pydantic.create_model` — dynamic endpoint generation from schemas, OpenAPI for free.
- **sqlglot** — SQL identifier parsing per dialect, DML building, dialect translation; check `dialect=` is actually passed.
- **pygls** — LSP position encoding (UTF-16 codec), `file://` URIs, workspace/buffer store, at the protocol edge only.
- **jinja2** — ad-hoc `{{ }}` substitution regexes, when template awareness matters.
- **construct** — parallel `struct` format strings, tuple indexing, packet size checks, and separate binary encoders/decoders.
- **crcmod** — hand-written CRC polynomial loops for named standard CRC variants.

## JavaScript — web platform

- **EventTarget** — local `Map<event, Set<handler>>` subscriber registries.
- **Intl.NumberFormat** — hand-written currency separators, rounding, grouping, and symbols.
- **HTMLDialogElement** — modal backdrops, Escape handling, inert backgrounds, focus trapping, and focus restoration.
- **CSS Grid/Flex, container/viewport units, and aspect-ratio** — resize listeners and hard-coded viewport arithmetic used only for layout.
- **Object.groupBy** — reduce loops that group records by a derived key, when the deployed browser baseline supports it.

## JavaScript — general

- **vue** (`Transition` and local components) — custom transition frameworks and repeated view-frame or presentation markup.
- **tailwindcss** — parallel component CSS for standard layout, controls, spinners, and pulse animations.
- **@microsoft/fetch-event-source** — duplicated EventSource connection, retry, abort, response validation, and event-stream parsing machinery.
- **xstate** — distributed workflow status strings, legal-transition checks, delayed callbacks, and async-effect lifecycle cleanup.
- **openapi-typescript** — hand-copied OpenAPI request/response vocabulary and drift-prone TypeScript declarations.
- **openapi-fetch** — hand-built endpoint strings, parameter placement, request serialization, and response typing over an authoritative OpenAPI contract.
- **@vueuse/core** — WebSocket reconnect loops plus component-owned timers, animation frames, media queries, keyboard chords, and event-listener cleanup.
- **@tanstack/vue-query** — repeated remote loading/error state, polling loops, request deduplication, and cache invalidation in Vue.
- **pinia-plugin-persistedstate** — manual localStorage synchronization and Pinia hydration plumbing.
- **lucide-vue-next** — hand-drawn and duplicated inline SVG icon inventories in Vue.

## Python — stdlib (added 2026-08)

- **string.Template** (subclassed, braces-only pattern) — hand-written `${NAME}` grammars: three regexes plus `is_valid`/`get_identifiers`/`safe_substitute` re-implementations.
- **enum.IntEnum** — rank ladders: frozen dataclass + `rank` field + name lookup dict + `max(key=lambda x: x.rank)`.
- **operator.lt/le/gt/ge** — four-arm `match` over a comparison-operator enum.
- **collections.OrderedDict** (`move_to_end` + `popitem(last=False)`) — per-instance LRU of N when `lru_cache` cannot key the call.

## Python — general (added 2026-08)

- **rich** (`Table.grid`) — hand-computed column widths, `f"{x:<{w}}"` padding, and continuation-line gutters in terminal reports.
- **pydantic.alias_generators.to_camel** + **AliasGenerator(serialization_alias=...)** — one `Field(serialization_alias="camelCase")` per wire field.
- **tzdata** — makes `zoneinfo.available_timezones()` deterministic; retires host-dependent time-zone vocabularies and import-time crashes on hosts without a system tz db.
- **croniter** — cron expression validation and next-fire-time: field/range/step/alias parsing and `@daily`-style presets.
- **litestar** + **msgspec** (or pydantic, which litestar also serialises natively) — hand-rolled ASGI routing, RFC 9457 problem details, OpenAPI; avoid keeping a second wire model type when the domain model already serialises.
- **adbc-driver-*** family — SQLAlchemy dialect packages for one more backend; pair with the sqlglot dialect that already exists.
- **datacontract-specification** — hand-typed pydantic models of the datacontract.com spec.
