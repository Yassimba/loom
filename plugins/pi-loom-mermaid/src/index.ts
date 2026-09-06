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
};

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
      const styledSource = withDiffClasses(token.text);
      const art = render(styledSource.source);
      if (!art || art.width > context.availableWidth) return token.raw;
      return `${dimDefaultBorders(toAnsi(art), styledSource.dimSgr).map(codeSpan).join("  \n")}\n`;
    })
    .join("");
}

export default function piLovelyMermaid(pi: ExtensionAPI): void {
  pi.registerMarkdownTransformer(transformMermaidMarkdown);
}
