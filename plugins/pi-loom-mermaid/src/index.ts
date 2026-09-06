import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Marked, type Token } from "@earendil-works/pi-tui";
import { render, toAnsi } from "./loom-mermaid/index.ts";

const markdownParser = new Marked();
const diffClasses = {
  red: { definition: "classDef red stroke:#9f5555", sgr: "38;2;159;85;85" },
  orange: { definition: "classDef orange stroke:#9a7438", sgr: "38;2;154;116;56" },
  green: { definition: "classDef green stroke:#4f8560", sgr: "38;2;79;133;96" },
};

type TransformContext = {
  messageType: "user" | "assistant" | "assistant-thinking";
  availableWidth: number;
  /** The message is still arriving; an unclosed fence shows as source until it closes. */
  isStreaming?: boolean;
};

/**
 * Rendered blocks by source and width. Pi runs the transformer on the whole
 * message for every streamed chunk and every redraw, so a diagram would be
 * laid out again for each token after it. Layout is deterministic, so the
 * first render is the only one needed.
 */
const rendered = new Map<string, string>();
const CACHE_SIZE = 64;

function renderBlock(text: string, availableWidth: number): string | null {
  const key = `${availableWidth}\0${text}`;
  const hit = rendered.get(key);
  if (hit !== undefined) {
    rendered.delete(key);
    rendered.set(key, hit);
    return hit;
  }
  const styledSource = withDiffClasses(text);
  const art = render(styledSource.source, { maxWidth: availableWidth });
  if (!art || art.width > availableWidth) return null;
  const out = `${dimDefaultBorders(toAnsi(art), styledSource.dimSgr).map(codeSpan).join("  \n")}\n`;
  rendered.set(key, out);
  if (rendered.size > CACHE_SIZE) rendered.delete(rendered.keys().next().value as string);
  return out;
}

/** A fenced block whose closing fence has arrived. */
const isClosed = (token: { raw: string }): boolean => /\n\s*(`{3,}|~{3,})\s*$/.test(token.raw);

function isMermaid(token: Token): token is Token & { type: "code"; text: string; lang?: string } {
  return (
    token.type === "code" && token.lang?.trim().split(/\s+/, 1)[0]?.toLowerCase() === "mermaid"
  );
}

function withDiffClasses(source: string): { source: string; dimSgr: string[] } {
  const defaults = Object.entries(diffClasses).filter(([name]) => {
    const used = new RegExp(`:::\\s*${name}\\b|\\bclass\\s+[^\\n]+\\s+${name}\\b`).test(source);
    return used && !new RegExp(`\\bclassDef\\s+${name}\\b`).test(source);
  });
  return {
    source:
      defaults.length > 0
        ? `${source}\n${defaults.map(([, style]) => style.definition).join("\n")}`
        : source,
    dimSgr: defaults.map(([, style]) => style.sgr),
  };
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

export function transformMermaidMarkdown(markdown: string, context: TransformContext): string {
  if (context.messageType === "assistant-thinking") return markdown;

  return markdownParser
    .lexer(markdown)
    .map((token) => {
      if (!isMermaid(token)) return token.raw;
      if (context.isStreaming === true && !isClosed(token)) return token.raw;
      return renderBlock(token.text, context.availableWidth) ?? token.raw;
    })
    .join("");
}

export default function piLovelyMermaid(pi: ExtensionAPI): void {
  pi.registerMarkdownTransformer(transformMermaidMarkdown);
}
