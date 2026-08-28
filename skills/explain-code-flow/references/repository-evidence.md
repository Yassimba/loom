# Repository evidence

Archify's contract for diagrams that must reflect real code. Its `repository-evidence.mjs` does no
code analysis: it only verifies that each cited `source` path and line range exists at a git
revision and builds a permalink. The analysis discipline is this paragraph:

> When the diagram must reflect real code, inspect repository entrypoints, runtime boundaries,
> storage, transports, and deployment configuration before authoring. Record only evidence you
> actually verified. Never infer runtime causality from file proximity or naming alone.

What to inspect, in order:

1. Entrypoints — main, handlers, routers, CLI commands, scheduled jobs.
2. Runtime boundaries — processes, containers, services, workers.
3. Storage — databases, caches, queues, object stores.
4. Transports — HTTP, gRPC, message buses, files, sockets.
5. Deployment configuration — compose, k8s, Terraform, CI.

Evidence rules:

- Every node and edge that claims to be real cites `file:line` you opened.
- A line number is copied from a `grep -n` / `sed -n` result, never estimated from a range you remember; report it as `path:N: <the line>` so it can be checked without reopening the file.
- A claim you did not verify is marked as an assumption, not drawn as fact.
- Proximity in the tree, shared naming, or an import alone is not a runtime call.

## Deliverables

Report each of these with current `file:line` anchors, call chains over prose, under 1,500 words:

- entry points and composition root
- layers and dependency direction
- core types, protocols, and their implementations
- domain entities with fields and relationships; persisted tables when present
- lifecycle states and the events between them, when present
- one end-to-end call chain with the data at each hop: shape in, shape out
- external systems and state changes
- modes, dispatch tables, and catalogs
- file and line counts
