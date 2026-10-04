import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { ansiLines, drawMessage, fenced, GUIDANCE } from "./shared.ts";

type TransformContext = {
  messageType: "user" | "assistant" | "assistant-thinking";
  availableWidth: number;
  /** Draw completed statements while the message is still arriving. */
  isStreaming?: boolean;
};

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
  return drawMessage(markdown, availableWidth, false)
    .map(({ raw, indent, drawing }) => {
      // A document fence has no indented form, so a list item's diagram stays source.
      if (drawing === null || drawing === "pending" || indent !== "") return raw;
      // The document viewer accepts SGR styling only, not terminal hyperlinks.
      const styled = drawing.art.styled.map((row) =>
        row.map((span) => ({ ...span, href: undefined })),
      );
      const text = ansiLines({ ...drawing, art: { ...drawing.art, styled } }).join("\n");
      return fenced(text, "loom-mermaid");
    })
    .join("");
}

export function transformMermaidMarkdown(markdown: string, context: TransformContext): string {
  if (context.messageType === "assistant-thinking") return markdown;

  return drawMessage(markdown, context.availableWidth, context.isStreaming === true)
    .map(({ raw, indent, drawing }) => {
      if (drawing === null) return raw;
      if (drawing === "pending") return "_Drawing Mermaid…_\n";
      const lines = ansiLines(drawing).map((line) => indent + codeSpan(line));
      return lines.join("  \n") + (indent === "" || raw.endsWith("\n") ? "\n" : "");
    })
    .join("");
}

export default function piLovelyMermaid(pi: ExtensionAPI): void {
  pi.registerMarkdownTransformer(transformMermaidMarkdown);
  pi.on("before_agent_start", (event) => ({
    systemPrompt: `${event.systemPrompt}\n\n${GUIDANCE}`,
  }));
}
