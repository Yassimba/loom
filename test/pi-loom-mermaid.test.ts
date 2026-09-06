import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { transformMermaidMarkdown } from "../plugins/pi-loom-mermaid/src/index.ts";
import { render } from "../plugins/pi-loom-mermaid/src/loom-mermaid/index.ts";

test("renders diff classes as dim colored borders without colored backgrounds", () => {
  const markdown = `\`\`\`mermaid
flowchart LR
  A[Removed]:::red --> B[Changed]:::orange --> C[Added]:::green
\`\`\``;

  const output = transformMermaidMarkdown(markdown, {
    messageType: "assistant",
    availableWidth: 200,
  });

  assert.match(output, /┌|╭|╔/);
  assert.ok(output.includes("\u001b[2;38;2;159;85;85m"));
  assert.ok(output.includes("\u001b[2;38;2;154;116;56m"));
  assert.ok(output.includes("\u001b[2;38;2;79;133;96m"));
  assert.doesNotMatch(output, /48;2/);
  assert.doesNotMatch(output, /classDef/);
});

test("vendored renderer handles every advertised diagram kind", () => {
  const diagrams = [
    "flowchart LR\n A --> B",
    "stateDiagram-v2\n A --> B",
    "classDiagram\n class A",
    "erDiagram\n A ||--o{ B : has",
    "sequenceDiagram\n A->>B: hello",
    'pie\n "A" : 1',
    "mindmap\n root((A))\n  B",
    "timeline\n 2026 : Shipped",
    "gitGraph\n commit",
  ];

  for (const source of diagrams) assert.ok(render(source), source.split("\n", 1)[0]);
});

test("a diagram wider than the space is laid out again with tighter labels", () => {
  const source = readFileSync(
    new URL("./fixtures/mermaid/skip-labelled.mmd", import.meta.url),
    "utf8",
  );
  const loose = render(source);
  const fitted = render(source, { maxWidth: 45 });
  assert.ok(loose && fitted);
  assert.ok(loose.width > 45);
  assert.ok(fitted.width <= 45);
  assert.equal(render(source)?.width, loose.width, "limits are restored after a retry");

  const output = transformMermaidMarkdown(`\`\`\`mermaid\n${source}\`\`\``, {
    messageType: "assistant",
    availableWidth: 45,
  });
  assert.doesNotMatch(output, /```mermaid/, "fits instead of falling back to source");
});

test("two edges passing through one cell cross as a hop, junctions stay junctions", () => {
  const art = render(
    readFileSync(new URL("./fixtures/mermaid/dense.mmd", import.meta.url), "utf8"),
  );
  assert.ok(art);
  const text = art.plain.join("\n");
  assert.match(text, /╫/, "a straight drop crossed by another edge's bus is a hop");
  assert.match(text, /┼/, "an edge continuing through its own bus row stays a junction");
});
