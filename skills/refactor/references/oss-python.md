# Python library candidates

Apply the selection rules in the [library catalog](oss-libraries.md). Verify each candidate against the project's supported versions and required semantics.

## Standard library

- **graphlib.TopologicalSorter** — hand-rolled topological sorts and load-order loops; `CycleError` identifies a cycle.
- **functools.lru_cache / cache** — single-site memoisation dicts when call keys and object lifetimes fit.
- **pathlib.Path.from_uri** (3.13+) — `file://` URI parsing; check platform-specific drive-letter, authority, and UNC behavior.
- **difflib.get_close_matches** — basic "did you mean" suggestions.
- **textwrap.wrap** — word-boundary wrapping, maximum line counts, and long-word/hyphen policies.
- **string.Template** (subclassed, braces-only pattern) — hand-written `${NAME}` grammars and separate validation, identifier discovery, and substitution implementations; check method availability.
- **enum.IntEnum** — numeric rank ladders with hand-written lookup and comparison code.
- **operator.lt/le/gt/ge** — four-arm dispatch over comparison operators.
- **collections.OrderedDict** (`move_to_end` + `popitem(last=False)`) — small per-instance LRU caches when `lru_cache` cannot express the key or lifetime.
- **threading.Event** — test polling loops waiting for a worker to reach a point; the worker signals and the test waits with a timeout.

## General

- **cachetools** — bounded LRU/TTL caches and eviction loops; check synchronization requirements.
- **stamina** — retry loops needing backoff; an opinionated wrapper over tenacity, if its retry semantics fit.
- **msgspec** — JSON/msgpack encode-decode and validation machinery when its model and schema support fit.
- **dacite** — `from_dict` factories that build nested dataclasses from mappings.
- **autoregistry** — registry dicts plus registration decorators keyed on class or function names.
- **lazy-loader** — PEP 562 lazy re-export tables repeated across type-checking imports, exports, and `__all__`.
- **typing_inspection** — PEP 695 alias unwrapping, union-arm extraction, and Literal/Optional introspection; check whether the project already declares it.
- **referencing** + **jsonschema** — JSON Schema reference resolution and keyword semantics.
- **pydantic** (when already a dependency) — model validation, discriminated unions, and JSON Schema generation; check coercion and serialization behavior.
- **pydantic** `model_validate(obj, from_attributes=True)` — factories that copy same-named fields from a dataclass or record into a wire model.
- **pydantic.alias_generators.to_camel** + **AliasGenerator(serialization_alias=...)** — repeated camel-case serialization aliases.
- **annotated-types** — Ge/Gt/Le/Lt/Interval constraint vocabularies consumed by compatible validators.
- **fast-depends / lagom / svcs** — DI containers with type-keyed registries, signature introspection, and cycle detection.
- **rapidfuzz** — fuzzy matching beyond difflib's quality or speed.
- **boltons** (fileutils) — atomic-write machinery; verify durability and metadata guarantees.
- **xattr** — extended-attribute access; verify platform support and handle ACL-copying requirements separately.
- **yoyo-migrations** — SQL migration ledgers and apply-pending orchestration.
- **deepdiff / DeepHash** — structural diffing and content hashing of nested objects.
- **intervaltree / sortedcontainers** — scans over span/interval or ordered collections.
- **more-itertools** — windowing, bucketing, and grouping recipes.
- **stevedore** — entry-point plugin loading and failure handling.
- **blinker / pluggy** — observer/event dispatch and hook systems when the authoring contract can fit.
- **ruamel.yaml** — YAML round-tripping with comments, anchors, merge keys, and source locations.
- **httpx** (+ httpx-auth) — HTTP transports needing a shared sync/async interface.
- **fastapi** + `pydantic.create_model` — schema-driven endpoint and OpenAPI machinery.
- **sqlglot** — SQL parsing, construction, and dialect translation; pass the intended dialect explicitly.
- **pygls** — LSP protocol machinery, including position and document handling; verify the specific API before replacing local behavior.
- **jinja2** — ad-hoc template substitution when template semantics are required.
- **construct** — parallel binary format strings, tuple indexing, packet checks, and separate encoders/decoders.
- **crcmod** — CRC polynomial loops for supported named variants.
- **rich** (`Table.grid`) — hand-computed terminal column widths, padding, and continuation gutters.
- **tzdata** — packaged IANA time-zone data for hosts without a system database; pinning it alone does not override system data used by `zoneinfo`.
- **croniter** — cron parsing and next-fire-time calculations; verify timezone, DST, and dialect semantics.
- **litestar** + **msgspec** (or pydantic) — ASGI routing, problem details, and OpenAPI machinery when the framework's contracts fit.
- **adbc-driver-*** family — Arrow-oriented database connectivity where its supported drivers and API meet the application's needs.
- **datacontract-specification** — hand-maintained models of the datacontract.com specification, when the required version matches.
