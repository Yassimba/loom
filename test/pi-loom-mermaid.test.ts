import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import piLovelyMermaid, { transformMermaidMarkdown } from "../plugins/pi-loom-mermaid/src/index.ts";
import { render, toAnsi } from "../plugins/pi-loom-mermaid/src/loom-mermaid/index.ts";

test("adds Mermaid guidance to the existing system prompt on each agent start", () => {
  let handler: ((event: { systemPrompt: string }) => { systemPrompt: string }) | undefined;
  piLovelyMermaid({
    registerMarkdownTransformer() {},
    on(event: string, callback: typeof handler) {
      if (event === "before_agent_start") handler = callback;
    },
  } as unknown as ExtensionAPI);

  assert.ok(handler);
  for (const base of ["Original instructions", "Updated instructions"]) {
    const { systemPrompt } = handler({ systemPrompt: base });
    assert.ok(systemPrompt.startsWith(`${base}\n\n`));
    assert.match(systemPrompt, /fenced `mermaid` blocks/);
    assert.match(
      systemPrompt,
      /flowchart, sequence, state, class, ER, mindmap, timeline, pie, and git graph/,
    );
    assert.match(systemPrompt, /Always visualize node diffs when it makes sense/);
    assert.match(
      systemPrompt,
      /`:::red` for removed, `:::green` for added, and `:::orange` for changed nodes/,
    );
  }
});

test("node fills stay inside the outline, never behind border glyphs", () => {
  for (const node of ["A[Help]", "A(Help)", "A{Help}"]) {
    const art = render(
      `flowchart TD\n ${node}:::custom\n classDef custom fill:#ecfdf5,stroke:#4f8560,color:#000`,
    );
    assert.ok(art);
    const lines = toAnsi(art);
    assert.doesNotMatch(lines[0], /48;2/, "top border has no background");
    assert.doesNotMatch(lines.at(-1) ?? "", /48;2/, "bottom border has no background");
    assert.match(lines[1], /48;2;236;253;245/, "interior retains its fill");
    for (const line of lines) {
      // biome-ignore lint/suspicious/noControlCharactersInRegex: inspect actual ANSI style runs
      for (const span of line.matchAll(/\u001b\[([^m]*)m([^\u001b]*)/g)) {
        if (span[1].includes("48;2")) {
          assert.doesNotMatch(span[2], /[┌┐└┘─│╭╮╰╯╔╗╚╝═║]/);
        }
      }
    }
  }
});

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

  for (const source of diagrams) {
    assert.ok(render(source), source.split("\n", 1)[0]);
    const live = transformMermaidMarkdown(`\`\`\`mermaid\n${source}\n`, {
      messageType: "assistant",
      availableWidth: 120,
      isStreaming: true,
    });
    assert.doesNotMatch(live, /```mermaid|Drawing Mermaid/, source);
  }
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

test("streaming advances on completed statements and holds while a label arrives", () => {
  const streaming = { messageType: "assistant" as const, availableWidth: 80, isStreaming: true };
  const prefix = "```mermaid\nflowchart TD\n  A[First] --> B[Second]\n";
  const first = transformMermaidMarkdown(prefix, streaming);
  assert.doesNotMatch(first, /```mermaid/);
  assert.match(first, /First/);
  for (const tail of [
    " B --",
    " B --> C[Thi",
    ' B --> C["Third;\nnode',
    " B --> C[Third\nnode",
    ' %% comment with " and ;',
  ]) {
    assert.equal(transformMermaidMarkdown(prefix + tail, streaming), first);
  }
  const next = `${prefix} B --> C[Third]\n`;
  assert.match(transformMermaidMarkdown(next, streaming), /Third/);
  const closed = `${next}\`\`\`\n`;
  assert.equal(
    transformMermaidMarkdown(next, streaming),
    transformMermaidMarkdown(closed, streaming),
  );
  const t = performance.now();
  for (let i = 0; i < 200; i++) transformMermaidMarkdown(`${prefix} B --> C[Thi`, streaming);
  assert.ok(performance.now() - t < 100, "partial-token updates reuse the completed prefix");
});

test("streaming supports semicolons and respects the opening fence length and character", () => {
  const streaming = { messageType: "assistant" as const, availableWidth: 80, isStreaming: true };
  for (const fence of ["```", "~~~~", "````"]) {
    const prefix = `${fence}mermaid\nflowchart TD; A[First] --> B[Second];`;
    const first = transformMermaidMarkdown(prefix, streaming);
    assert.match(first, /First/);
    assert.doesNotMatch(first, /mermaid/);
    assert.equal(transformMermaidMarkdown(`${prefix} B --> C[Incomplete`, streaming), first);
    assert.equal(transformMermaidMarkdown(`${prefix}\n~~`, streaming), first);
    assert.equal(transformMermaidMarkdown(`${prefix}\n\`\``, streaming), first);
  }
});

test("streaming has no diagram leakage across blocks and keeps final fallbacks", () => {
  const streaming = { messageType: "assistant" as const, availableWidth: 80, isStreaming: true };
  const first = "```mermaid\nflowchart TD\n A[First]\n```\n\n";
  const output = transformMermaidMarkdown(
    `${first}\`\`\`mermaid\nflowchart TD\n B[Unfinished`,
    streaming,
  );
  assert.equal(output.match(/First/g)?.length, 1);
  assert.doesNotMatch(output, /Unfinished/);
  for (const markdown of ["```mermaid\nunsupported\n A --> B", "```js\nconst x = 1"]) {
    assert.equal(transformMermaidMarkdown(markdown, streaming), markdown);
  }
  const invalid = "```mermaid\nflowchart TD\n```";
  assert.equal(transformMermaidMarkdown(invalid, { ...streaming, isStreaming: false }), invalid);
  assert.equal(
    transformMermaidMarkdown(first, { ...streaming, messageType: "assistant-thinking" }),
    first,
  );
});

test("explicit fill, stroke and text colors are preserved without tinting", () => {
  const markdown =
    "```mermaid\nflowchart TD\n A[Help text]:::custom\n classDef custom fill:#ecfdf5,stroke:#4f8560,color:#000\n```";
  const output = transformMermaidMarkdown(markdown, {
    messageType: "assistant",
    availableWidth: 80,
  });
  assert.ok(output.includes("48;2;236;253;245"), "literal author fill");
  assert.ok(output.includes("38;2;79;133;96"), "literal author stroke");
  assert.ok(output.includes("38;2;0;0;0"), "literal author text");
});
