import type { Register, TextProps } from "claude-code";
import { resolveClassStyle } from "../src/loom-mermaid/class-style.ts";
import type { Role } from "../src/loom-mermaid/types.ts";
import { type Drawn, drawMessage, fenced, GUIDANCE } from "../src/shared.ts";

/** A surface that has not measured draws at the document transformer's width. */
const DEFAULT_COLUMNS = 100;

/** Dim frame, plain labels, cyan connectors: the renderer's ANSI theme as Text props. */
const THEME: Partial<Record<Role, TextProps>> = {
  border: { dimColor: true },
  edge: { color: "cyan" },
  edgeLabel: { color: "cyan", dimColor: true },
  title: { bold: true },
};

function runs({ art, dimStrokes }: Drawn) {
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
 * A streaming reply as its lines are shown: each closed fence as an uncolored
 * drawing, a fence still open withheld. Lines only ever add to the end of it,
 * so what a new batch shows is what it adds; the finished reply is then drawn
 * in color by the `AssistantMessage` site.
 */
function shown(text: string, columns: number): string {
  return drawMessage(text, columns, false)
    .map(({ raw, indent, drawing, open }) => {
      if (open) return "";
      if (drawing === null || drawing === "pending") return raw;
      return fenced(drawing.art.plain.join("\n")).replace(/^(?=.)/gm, indent);
    })
    .join("");
}

export const register: Register = (on) => {
  // The streaming event carries no width, so it draws at the last one a reply was drawn at.
  let columns = DEFAULT_COLUMNS;
  // ponytail: a reply that never sends its final batch keeps its text here; cap it if sessions leak.
  const arriving = new Map<string, string>();

  on("classic.MessageDisplay", async (_$, e, next) => {
    const result = await next(e);
    const before = arriving.get(e.message_id) ?? "";
    const text = before + e.delta;
    if (e.final) arriving.delete(e.message_id);
    else arriving.set(e.message_id, text);
    const displayContent = shown(text, columns).slice(shown(before, columns).length);
    return displayContent === e.delta ? result : { ...result, displayContent };
  });

  on("prompt.compose", async (_$, e, next) => {
    const { sections } = await next(e);
    return {
      sections: [...sections, { id: "loom-mermaid:guidance", text: GUIDANCE, scope: "session" }],
    };
  });

  on("ui.render", { component: "AssistantMessage" }, ($, e, next) => {
    // The site carries no streaming flag, so an unclosed fence is one still arriving.
    columns = e.viewport?.columns ?? DEFAULT_COLUMNS;
    const parts = drawMessage(e.props.text, columns, true);
    if (parts.every((part) => part.drawing === null)) return next(e);

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
              {runs(drawing).map((row, y) => (
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
