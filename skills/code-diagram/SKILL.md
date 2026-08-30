---
name: code-diagram
description: "Generate a strict Review-style walkthrough with SequenceDiagram, CallStackDiff, DatabaseLens, and SoftwareMap as one source-bound offline HTML file. Use for runtime order, stack changes, durable-store behavior, or repository architecture that needs clickable exact source evidence."
---

# Code Diagram

Author `review.mdx` plus typed `data.ts`. Optionally add typed `software-map.ts`; SoftwareMap is a separate Review artifact, not an MDX component. `check` and `build` use the same strict compiler path.

## Author typed data

```ts
import {
  calls,
  defineActors,
  defineAnchors,
  defineStores,
  defineSoftwareActors,
  defineSoftwareStores,
} from "virtual:progressive-review-authoring";
```

- `defineActors` creates stable actor references.
- `defineAnchors` creates source-bound references. Set `graph: "base"` only for removed CallStackDiff frames; head/default reads the dirty worktree.
- `calls(parent, child, reason)` annotates an asynchronous or otherwise non-local hop without changing reference identity.
- `defineStores` creates typed relational or document collection/field targets for DatabaseLens.
- `defineSoftwareActors(model, refs)` and `defineSoftwareStores(model, refs)` derive typed references from a normalized software map.

Every `softwareMapPath` must exist in the adjacent `software-map.ts`. Do not cast through `any`. Every Sequence message and database operation needs a peekable anchor or, for Sequence only, inline `code`.

## Author `review.mdx`

Use built-ins without importing them:

```mdx
import { base, head, messages, stores, actors, anchors } from "./data.ts";

# Request flow

<SequenceDiagram label="Submit" messages={messages} />
<CallStackDiff title="Dispatch" base={base} head={head} />
<DatabaseLens title="Persistence" stores={stores}>
  <DbUseCase id="save" label="Save request">
    <DbWrite from={actors.api} to={stores.db.documents.requests.status} label="persist status" anchor={anchors.save} />
  </DbUseCase>
</DatabaseLens>
```

`DbUseCase` must be a direct DatabaseLens child. `DbRead` and `DbWrite` must be direct use-case children. Reads flow target → actor; writes flow actor → target.

## Author optional `software-map.ts`

Use Review's map import and default-export the normalized map:

```ts
import { defineSoftwareMap } from "@dev.fast/progressive-review/software-map-model";

export default defineSoftwareMap({
  systems: {
    app: {
      containers: {
        api: {
          components: {
            handler: {
              codeElements: {
                route: { sourceRanges: [{ file: "src/api.ts", fromLine: 10, toLine: 24 }] },
              },
            },
          },
        },
      },
    },
  },
});
```

Element IDs become stable dot paths. Relationships must reference existing paths. Missing `software-map.ts` simply omits the map; an invalid artifact fails `check`.

## Check and build

Resolve the script relative to this skill directory:

```bash
node --import tsx <skill>/scripts/code-diagram.ts check review.mdx --repo <repo-root>
node --import tsx <skill>/scripts/code-diagram.ts build review.mdx \
  --repo <repo-root> --out ai-docs/diagrams/<name>.html
```

The checker type-checks both authored TypeScript files, compiles MDX, validates strict props/grammar/map structure, resolves dirty-head or pinned-base source, and verifies CallStackDiff `-`/`+` claims against `git diff HEAD`.

The output embeds MDX, CSS, JavaScript, compiled models, and exact source lines. Source-linked clicks dispatch `code-diagram:open-source` on `window` with `{ sources, title }`; the host owns the editor UI. Map selections include every source range under the selected node. Removed map elements read source from the pinned base.

The file loads from `file://` with a deny-by-default CSP and no server, external asset, network request, comments, telemetry, or session persistence. Executable authored HTML is rejected before output.

See `examples/loom-installer/` for all four surfaces in one document.
