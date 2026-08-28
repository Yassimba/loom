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
- A claim you did not verify is marked as an assumption, not drawn as fact.
- Proximity in the tree, shared naming, or an import alone is not a runtime call.
