import type { Register, TextProps } from "claude-code";
import { resolveClassStyle } from "../src/loom-mermaid/class-style.ts";
import type { Role } from "../src/loom-mermaid/types.ts";
import { type Drawn, drawMessage, GUIDANCE } from "../src/shared.ts";

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

export const register: Register = (on) => {
  on("prompt.compose", async (_$, e, next) => {
    const { sections } = await next(e);
    return {
      sections: [...sections, { id: "loom-mermaid:guidance", text: GUIDANCE, scope: "session" }],
    };
  });

  on("ui.render", { component: "AssistantMessage" }, ($, e, next) => {
    // The site carries no streaming flag, so an unclosed fence is one still arriving.
    const parts = drawMessage(e.props.text, e.viewport?.columns ?? DEFAULT_COLUMNS, true);
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
