import type { Register, TextProps } from "claude-code";
import { type Segment, scanFences } from "../src/fences.ts";
import { resolveClassStyle } from "../src/loom-mermaid/class-style.ts";
import { diagramKind, render } from "../src/loom-mermaid/index.ts";
import type { MermaidArt, Role } from "../src/loom-mermaid/types.ts";
import { GUIDANCE, withDiffClasses } from "../src/shared.ts";
import { streamingPrefixes } from "../src/streaming.ts";

type Fence = NonNullable<Segment["mermaid"]>;
type Run = { text: string; href?: string; style: TextProps };
type Drawing = Run[][] | "pending" | null;

/** A surface that has not measured draws at the document transformer's width. */
const DEFAULT_COLUMNS = 100;
/** The longest text one `Markdown` element takes. */
const MARKDOWN_LIMIT = 10000;

/** Dim frame, plain labels, cyan connectors: the renderer's ANSI theme as Text props. */
const THEME: Partial<Record<Role, TextProps>> = {
  border: { dimColor: true },
  edge: { color: "cyan" },
  edgeLabel: { color: "cyan", dimColor: true },
  title: { bold: true },
};

function runs(art: MermaidArt, dimStrokes: string[]): Run[][] {
  return art.styled.map((row) =>
    row.map((span) => {
      const themed = THEME[span.role] ?? {};
      const cls = resolveClassStyle(span.classes, art.classDefs);
      // Only `stroke` colors a border; fills and text colors stay with the theme.
      const stroke = span.role === "border" ? cls?.stroke : undefined;
      const style =
        stroke === undefined
          ? { ...themed, ...(cls?.bold === true ? { bold: true } : {}) }
          : { color: stroke, bold: cls?.bold === true, dimColor: dimStrokes.includes(stroke) };
      return { text: span.text, href: span.href, style };
    }),
  );
}

/**
 * Drawings by source and width. The site is drawn again on every resize,
 * scroll and arriving chunk, and layout is deterministic.
 */
const drawn = new Map<string, Run[][] | null>();
const CACHE_SIZE = 64;

function drawSource(source: string, columns: number): Run[][] | null {
  const key = `${columns}\0${source.trimEnd()}`;
  const hit = drawn.get(key);
  if (hit !== undefined) return hit;
  const styled = withDiffClasses(source);
  const art = render(styled.source, { maxWidth: columns });
  const out = !art || art.width > columns ? null : runs(art, styled.dimStrokes);
  drawn.set(key, out);
  if (drawn.size > CACHE_SIZE) drawn.delete(drawn.keys().next().value as string);
  return out;
}

/** A fence still arriving draws its newest complete statements; null keeps the source. */
function draw(fence: Fence, columns: number): Drawing {
  if (fence.closed) return drawSource(fence.source, columns);
  if (fence.nested || diagramKind(fence.source) === null) return null;
  for (const prefix of streamingPrefixes(fence.source)) {
    const out = drawSource(prefix, columns);
    if (out !== null) return out;
  }
  return "pending";
}

export const register: Register = (on) => {
  on("prompt.compose", async (_$, e, next) => {
    const { sections } = await next(e);
    return {
      sections: [...sections, { id: "loom-mermaid:guidance", text: GUIDANCE, scope: "session" }],
    };
  });

  on("ui.render", { component: "AssistantMessage" }, ($, e, next) => {
    const segments = scanFences(e.props.text);
    if (!segments.some((segment) => segment.mermaid)) return next(e);
    const columns = e.viewport?.columns ?? DEFAULT_COLUMNS;
    const parts = segments.map(({ raw, mermaid }) => ({
      raw,
      indent: mermaid?.indent ?? "",
      drawing: mermaid ? draw(mermaid, columns - mermaid.indent.length) : null,
    }));
    const isDrawn = parts.some((part) => part.drawing !== null);
    const fits = parts.every((part) => part.drawing !== null || part.raw.length <= MARKDOWN_LIMIT);
    if (!isDrawn || !fits) return next(e);

    const { Box, Link, Markdown, Text } = $.ui.resolve(e);
    return (
      <Box flexDirection="column" gap={1}>
        {parts.map(({ raw, indent, drawing }, at) => {
          if (drawing === null) {
            return raw.trim() === "" ? null : <Markdown key={`text-${at}`} text={raw.trimEnd()} />;
          }
          if (drawing === "pending") {
            return (
              <Text key={`pending-${at}`} italic>
                Drawing Mermaid…
              </Text>
            );
          }
          return (
            <Box key={`diagram-${at}`} flexDirection="column">
              {drawing.map((row, y) => (
                <Text key={`row-${y}`} wrap="truncate">
                  {indent}
                  {row.map((run, x) => (
                    <Text key={`run-${x}`} {...run.style}>
                      {run.href === undefined || run.text.trim() === "" ? (
                        run.text
                      ) : (
                        <Link href={run.href}>{run.text}</Link>
                      )}
                    </Text>
                  ))}
                </Text>
              ))}
            </Box>
          );
        })}
      </Box>
    );
  });
};
