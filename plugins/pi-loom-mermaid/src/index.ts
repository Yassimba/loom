import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { scanFences } from "./fences.ts";
import { toAnsi } from "./loom-mermaid/index.ts";
import { type Drawn, drawDiagram, GUIDANCE } from "./shared.ts";
import { drawArriving } from "./streaming.ts";

type TransformContext = {
  messageType: "user" | "assistant" | "assistant-thinking";
  availableWidth: number;
  /** Draw completed statements while the message is still arriving. */
  isStreaming?: boolean;
};

function renderBlock(text: string, availableWidth: number): string | null {
  const drawn = drawDiagram(text, availableWidth);
  return drawn && `${ansiLines(drawn).map(codeSpan).join("  \n")}\n`;
}

/** The art as ANSI lines, its default diff borders dim. */
function ansiLines({ art, dimStrokes }: Drawn): string[] {
  const sgrValues = dimStrokes.map(
    (hex) => `38;2;${[1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16)).join(";")}`,
  );
  return toAnsi(art).map((line) =>
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
      const drawn = drawDiagram(mermaid.source, availableWidth);
      if (drawn === null) return raw;
      // The document viewer accepts SGR styling only, not terminal hyperlinks.
      const styled = drawn.art.styled.map((row) =>
        row.map((span) => ({ ...span, href: undefined })),
      );
      const text = ansiLines({ ...drawn, art: { ...drawn.art, styled } }).join("\n");
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
        const out = drawArriving(mermaid.source, (source) =>
          renderBlock(source, context.availableWidth),
        );
        return out === "pending" ? "_Drawing Mermaid…_\n" : (out ?? raw);
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
