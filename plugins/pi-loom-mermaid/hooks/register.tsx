import type { Elements, Register, RenderSurface, TextProps } from "claude-code";
import { atom, read, update } from "claude-code";
import { resolveClassStyle } from "../src/loom-mermaid/class-style.ts";
import type { Role } from "../src/loom-mermaid/types.ts";
import { ansiLines, type Drawn, drawMessage, fenced, GUIDANCE } from "../src/shared.ts";

/** A surface that has not measured draws at the document transformer's width. */
const DEFAULT_COLUMNS = 100;
/** The columns the transcript keeps for a reply's bullet. */
const GUTTER = 2;

/** Dim frame, plain labels, cyan connectors: the renderer's ANSI theme as Text props. */
const THEME: Partial<Record<Role, TextProps>> = {
  border: { dimColor: true },
  edge: { color: "cyan" },
  edgeLabel: { color: "cyan", dimColor: true },
  title: { bold: true },
};

function runs({ art }: Drawn) {
  return art.styled.map((row) =>
    row.map((span) => {
      const themed = THEME[span.role] ?? {};
      const cls = resolveClassStyle(span.classes, art.classDefs);
      // Only `stroke` colors a border; fills and text colors stay with the theme.
      // It is never drawn dim: `dimColor` replaces a Text's color with the theme's grey.
      const stroke = span.role === "border" ? cls?.stroke : undefined;
      const style =
        stroke === undefined
          ? { ...themed, ...(cls?.bold === true ? { bold: true } : {}) }
          : { color: stroke, bold: cls?.bold === true };
      return { text: span.text, href: span.href, style };
    }),
  );
}

const PENDING = "Drawing Mermaid…";

/** The diagram as one Text per row, each run in its own style. */
function diagram(
  { Box, Link, Text }: Pick<Elements[RenderSurface], "Box" | "Link" | "Text">,
  drawn: Drawn,
  indent: string,
  key: string,
) {
  return (
    <Box key={key} flexDirection="column">
      {runs(drawn).map((row, y) => (
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
}

/**
 * A streaming reply as its lines are shown: a Mermaid fence is withheld while
 * open and shows as its drawing once it closes. Lines only ever add to the end
 * of it, so what a new batch shows is what it adds; the band above the prompt
 * shows the fence still open, and the finished reply is drawn again by the
 * `AssistantMessage` site.
 */
function shown(text: string, columns: number): string {
  return drawMessage(text, columns, false)
    .map(({ raw, indent, drawing, fence }) => {
      if (fence === "open") return "";
      if (drawing === null || drawing === "pending") return raw;
      return fenced(ansiLines(drawing).join("\n")).replace(/^(?=.)/gm, indent);
    })
    .join("");
}

const arrivingFence = atom({ plugin: "loom-mermaid", key: "arriving" } as const, null);

export const register: Register = (on) => {
  // The streaming event carries no width, so it draws at the last one a site was drawn at.
  let columns = DEFAULT_COLUMNS;
  // ponytail: a reply that never sends its final batch keeps its text here; cap it if sessions leak.
  const arriving = new Map<string, string>();

  on("classic.MessageDisplay", async ($, e, next) => {
    const result = await next(e);
    const before = arriving.get(e.message_id) ?? "";
    const text = before + e.delta;
    if (e.final) arriving.delete(e.message_id);
    else arriving.set(e.message_id, text);

    const last = drawMessage(text, columns, true).at(-1);
    const open = !e.final && last?.fence === "open" ? last.raw : null;
    if ((await read($, arrivingFence)) !== open) await update($, arrivingFence, () => open);

    const width = columns - GUTTER;
    const displayContent = shown(text, width).slice(shown(before, width).length);
    return displayContent === e.delta ? result : { ...result, displayContent };
  });

  // The open fence of a streaming reply, growing statement by statement above the prompt.
  on("ui.render", { component: "AbovePrompt" }, async ($, e, next) => {
    columns = e.viewport?.columns ?? columns;
    const fence = await read($, arrivingFence);
    if (fence === null || e.props.hasSurvey) return next(e);
    const drawing = drawMessage(fence, e.props.bodyColumns, true)[0]?.drawing ?? null;
    if (drawing === null) return next(e);
    const ui = $.ui.resolve(e);
    return drawing === "pending" ? (
      <ui.Text italic>{PENDING}</ui.Text>
    ) : (
      diagram(ui, drawing, "", "preview")
    );
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
    const gutter = e.surface === "terminal" ? GUTTER : 0;
    const parts = drawMessage(e.props.text, columns - gutter, true);
    if (parts.every((part) => part.drawing === null)) return next(e);

    const ui = $.ui.resolve(e);
    const { Box, Markdown, Text } = ui;
    const body = (
      <Box flexDirection="column" gap={1}>
        {parts.map(({ raw, indent, drawing }, at) => {
          if (drawing === null) {
            return raw.trim() === "" ? null : <Markdown key={`text-${at}`} text={raw.trimEnd()} />;
          }
          if (drawing === "pending") {
            return (
              <Text key={`pending-${at}`} italic>
                {PENDING}
              </Text>
            );
          }
          return diagram(ui, drawing, indent, `diagram-${at}`);
        })}
      </Box>
    );
    if (e.surface !== "terminal") return body;
    // On the terminal a hook that draws the reply draws its chrome too: the
    // blank row above it, the bullet and the gutter.
    return (
      <Box flexDirection="row" marginTop={1}>
        <Text>{e.props.isFirstOfReply ? "⏺ " : "  "}</Text>
        {body}
      </Box>
    );
  });
};
