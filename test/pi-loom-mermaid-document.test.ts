import assert from "node:assert/strict";
import test from "node:test";
import { transformMermaidForDocument } from "../plugins/pi-loom-mermaid/src/index.ts";

test("renders Mermaid as a text code block for document viewers", () => {
  const markdown = "Before\n\n```mermaid\nflowchart LR\n A --> B\n```\n\nAfter\n";
  const output = transformMermaidForDocument(markdown, 80);

  assert.match(output, /^Before\n\n```\n[┌╭]/m);
  assert.doesNotMatch(output, /```text/);
  assert.doesNotMatch(output, /```mermaid/);
  assert.match(output, /\n\nAfter\n$/);
});

test("keeps unsupported Mermaid source intact", () => {
  const markdown = "```mermaid\nunsupported\n A --> B\n```\n";
  assert.equal(transformMermaidForDocument(markdown), markdown);
});
