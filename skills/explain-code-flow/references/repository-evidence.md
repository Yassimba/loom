# Repository evidence contract

Map real runtime behavior. Inspect in order:

1. executable entry and composition root;
2. process/service/thread boundaries;
3. stores, caches, queues, and files;
4. HTTP, RPC, buses, sockets, and other transports;
5. deployment or CI configuration when it affects the feature.

## Evidence rules

- Every factual node and edge cites a source line you opened.
- Copy line numbers from `grep -n` or `sed -n`; report `path:N: source line`.
- Mark unverified claims as assumptions.
- File proximity, naming, and imports do not prove runtime causality.
- Separate production reachability from tests. Tests may corroborate behavior, never serve as the production entry.

## Deliverable

Return a compact call-chain packet, not a repository tour:

```markdown
Boundary: <entry> → <final effect or composition gap>
Entrypoints: <symbol — path:line>
Runtime boundaries: <process/thread/service>
Types: <type → implementation — path:line>
Entities/tables: <fields, cardinality, persistence — path:line>  # only when real
Spine:
1. <function — value in → value out/state change — path:line>
Externals: <system/transport — path:line>
States: <state --event/guard--> state — path:line>  # only when real
Modes/catalogs: <dispatch table — path:line>        # only when real
Counts: <relevant files and lines>
```

Include only facts a figure or anchored walkthrough needs. Omit field inventories and per-key branches unless they change a figure. Budget: ≤600 words for 1–5 relevant files, ≤900 for 6–10, ≤1,200 for 11+. The main agent, not this worker, chooses figures.