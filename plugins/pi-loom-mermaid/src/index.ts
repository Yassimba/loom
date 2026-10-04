import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { scanFences } from "./fences.ts";
import { diagramKind, render, toAnsi } from "./loom-mermaid/index.ts";
import { GUIDANCE, withDiffClasses } from "./shared.ts";
import { streamingPrefixes } from "./streaming.ts";

type TransformContext = {
  messageType: "user" | "assistant" | "assistant-thinking";
  availableWidth: number;
  /** Draw completed statements while the message is still arriving. */
  isStreaming?: boolean;
};

/**
 * Rendered blocks by source and width. Pi runs the transformer on the whole
 * message for every streamed chunk and every redraw, so a diagram would be
 * laid out again for each token after it. Layout is deterministic, so the
 * first render is the only one needed.
 */
const rendered = new Map<string, string | null>();
const CACHE_SIZE = 64;

function renderBlock(text: string, availableWidth: number): string | null {
  const key = `${availableWidth}\0${text.trimEnd()}`;
  const hit = rendered.get(key);
  if (hit !== undefined) {
    rendered.delete(key);
    rendered.set(key, hit);
    return hit;
  }
  const styledSource = withDiffClasses(text);
  const art = render(styledSource.source, { maxWidth: availableWidth });
  const out =
    !art || art.width > availableWidth
      ? null
      : `${dimDefaultBorders(toAnsi(art), styledSource.dimSgr).map(codeSpan).join("  \n")}\n`;
  rendered.set(key, out);
  if (rendered.size > CACHE_SIZE) rendered.delete(rendered.keys().next().value as string);
  return out;
}

function dimDefaultBorders(lines: string[], sgrValues: string[]): string[] {
  return lines.map((line) =>
    sgrValues.reduce(
      (result, sgr) => result.replaceAll(`\u001b[${sgr}m`, `\u001b[2;${sgr}m`),
      line,
    ),
  );
}

function codeSpan(line: string): string {
  const content = line || "\u00a0";
  const longestRun = Math.max(
    0,
    ...Array.from(content.matchAll(/`+/g), (match) => match[0].length),
  );
  const fence = "`".repeat(longestRun + 1);
  const padding = content.startsWith("`") || content.endsWith("`") ? " " : "";
  return `${fence}${padding}${content}${padding}${fence}`;
}

export function transformMermaidForDocument(markdown: string, availableWidth = 100): string {
  return scanFences(markdown)
    .map(({ raw, mermaid }) => {
      if (!mermaid || mermaid.nested) return raw;
      const styledSource = withDiffClasses(mermaid.source);
      const art = render(styledSource.source, { maxWidth: availableWidth });
      if (!art || art.width > availableWidth) return raw;
      // The document viewer accepts SGR styling only, not terminal hyperlinks.
      const withoutLinks = {
        ...art,
        styled: art.styled.map((row) => row.map((span) => ({ ...span, href: undefined }))),
      };
      const text = dimDefaultBorders(toAnsi(withoutLinks), styledSource.dimSgr).join("\n");
      const longestRun = Math.max(
        0,
        ...Array.from(text.matchAll(/`+/g), (match) => match[0].length),
      );
      const fence = "`".repeat(Math.max(3, longestRun + 1));
      return `${fence}loom-mermaid\n${text}\n${fence}\n`;
    })
    .join("");
}

export function transformMermaidMarkdown(markdown: string, context: TransformContext): string {
  if (context.messageType === "assistant-thinking") return markdown;

  return scanFences(markdown)
    .map(({ raw, mermaid }) => {
      if (!mermaid) return raw;
      if (mermaid.nested) {
        const out = mermaid.closed ? renderBlock(mermaid.source, context.availableWidth) : null;
        if (out === null) return raw;
        const lines = out.trimEnd().split("\n");
        return (
          lines.map((line) => mermaid.indent + line).join("\n") + (raw.endsWith("\n") ? "\n" : "")
        );
      }
      if (context.isStreaming === true && !mermaid.closed) {
        if (diagramKind(mermaid.source) === null) return raw;
        for (const prefix of streamingPrefixes(mermaid.source)) {
          const out = renderBlock(prefix, context.availableWidth);
          if (out !== null) return out;
        }
        return "_Drawing Mermaid…_\n";
      }
      return renderBlock(mermaid.source, context.availableWidth) ?? raw;
    })
    .join("");
}

export default function piLovelyMermaid(pi: ExtensionAPI): void {
  pi.registerMarkdownTransformer(transformMermaidMarkdown);
  pi.on("before_agent_start", (event) => ({
    systemPrompt: `${event.systemPrompt}\n\n${GUIDANCE}`,
  }));
}
