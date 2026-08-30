---
name: code-diagram
description: "Generate a Review-style, source-bound sequence diagram as one offline HTML document. Use for code runtime order or message flow that needs clickable exact source evidence. Sequence Diagram is the only supported surface in this preview."
---

# Code Diagram

Build one source-bound sequence document. The shipped CLI writes a self-contained HTML file with Review's lane ordering, message routing, tour, and source panel.

This preview supports `type: "sequence"` only. Route other diagram types to their existing skill until later parity phases land.

## 1. Author

Write a trusted local `.mjs` file that default-exports this shape:

```js
export default {
  version: 1,
  title: "Request flow",
  intro: ["One sentence that tells the reader what crosses the diagram."],
  diagrams: [{
    type: "sequence",
    label: "Submit request",
    actors: {
      client: { label: "Client" },
      api: { label: "API" },
    },
    messages: [{
      from: "client",
      to: "api",
      label: "submit(input)",
      evidence: { file: "src/client.ts", fromLine: 20, toLine: 24 },
    }],
  }],
};
```

Use `scripts/authoring.d.ts` for the TypeScript contract. The optional `defineDocument` helper lives in `scripts/authoring.mjs`.

Every message needs either exact repository-relative `evidence` or inline `code`. Use source evidence for factual production behavior. Inline code is for an authored snippet already visible to the reader.

Done: actor IDs resolve, parallel messages have distinct labels, and every factual message has evidence.

## 2. Check

Resolve `scripts/code-diagram.mjs` relative to this skill directory, then run:

```bash
node <skill>/scripts/code-diagram.mjs check diagram.mjs --repo <repo-root>
```

Repair every diagnostic. The checker rejects unknown fields, unsupported diagram types, missing endpoints, duplicate identities, missing files, and stale line ranges.

Done: exit 0.

## 3. Build

```bash
node <skill>/scripts/code-diagram.mjs build diagram.mjs \
  --repo <repo-root> \
  --out ai-docs/diagrams/<name>.html
```

The HTML contains its scripts, styles, semantic model, and declared source ranges. It loads from `file://` without a server or network request.

Done: the command reports one output path; open it and verify each message opens the intended source lines.

## Boundary

The CLI executes `.mjs` input as trusted local code. Labels and source contents are escaped before browser rendering. Only declared source ranges enter the HTML.
