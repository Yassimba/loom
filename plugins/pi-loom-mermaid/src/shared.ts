/** What Pi and the Claude Code mod both do around the renderer. */

import { type Fence, scanFences } from "./fences.ts";
import { render, toAnsi } from "./loom-mermaid/index.ts";
import type { MermaidArt } from "./loom-mermaid/types.ts";
import { drawArriving } from "./streaming.ts";

/** The system prompt note that makes the model reach for Mermaid and the diff markers. */
export const GUIDANCE = `Use fenced \`mermaid\` blocks; they render automatically in the user’s session. Always use Mermaid when it is easier to read than prose. Choose by subject: architecture for deployed services, flowchart for dependencies/decisions, sequence for interactions, state for lifecycles, ER/class for models, mindmap for hierarchies, timeline/git graph for history, pie for proportions. Use complementary diagrams when explaining multiple aspects. Keep diagram labels short. Mark changes by appending :::red (removed), :::green (added), or :::orange (changed) after the node, outside its label brackets (A[Added]:::green, never A[Added :::green]); their default colors are automatic—do not add classDef for these diff markers. In general prefer colored outlines to logically group things (if there are no changes involved).`;

/** The default stroke of each diff marker. */
const diffStrokes = { red: "#9f5555", orange: "#9a7438", green: "#4f8560" };

/** Give the diff markers a source uses but does not define their default strokes. */
function withDiffClasses(source: string): { source: string; dimStrokes: string[] } {
  const defaults = Object.entries(diffStrokes).filter(([name]) => {
    const used = new RegExp(`:::\\s*${name}\\b|\\bclass\\s+[^\\n]+\\s+${name}\\b`).test(source);
    return used && !new RegExp(`\\bclassDef\\s+${name}\\b`).test(source);
  });
  return {
    source: [source, ...defaults.map(([name, stroke]) => `classDef ${name} stroke:${stroke}`)].join(
      "\n",
    ),
    dimStrokes: defaults.map(([, stroke]) => stroke),
  };
}

/** A diagram's art, and the default diff strokes it draws dim. */
export type Drawn = { art: MermaidArt; dimStrokes: string[] };

/**
 * Drawings by source and width. Both hosts draw a whole message again for
 * every streamed chunk and every redraw, and layout is deterministic, so the
 * first one is the only one needed.
 */
const drawn = new Map<string, Drawn | null>();
const CACHE_SIZE = 64;

/** The diagram drawn within `columns`, or null when the source does not draw or fit. */
function drawDiagram(source: string, columns: number): Drawn | null {
  const key = `${columns}\0${source.trimEnd()}`;
  const hit = drawn.get(key);
  if (hit !== undefined) {
    drawn.delete(key);
    drawn.set(key, hit);
    return hit;
  }
  const styled = withDiffClasses(source);
  const art = render(styled.source, { maxWidth: columns });
  const out = !art || art.width > columns ? null : { art, dimStrokes: styled.dimStrokes };
  drawn.set(key, out);
  if (drawn.size > CACHE_SIZE) drawn.delete(drawn.keys().next().value as string);
  return out;
}

/**
 * One run of a message: text to show as written (`drawing` null), a diagram,
 * or "pending" for a fence that has arrived too little to draw. A fence in a
 * list item carries the item's `indent`.
 */
export type Part = {
  raw: string;
  indent: string;
  drawing: Drawn | "pending" | null;
  /** Whether the run is a Mermaid fence, and whether its closing line has arrived. */
  fence: "open" | "closed" | null;
};

function draw(fence: Fence, columns: number, arriving: boolean): Part["drawing"] {
  if (fence.closed || !(arriving || fence.nested)) return drawDiagram(fence.source, columns);
  // A list item's fence draws only once closed.
  return fence.nested ? null : drawArriving(fence.source, (source) => drawDiagram(source, columns));
}

/**
 * Split a message at its Mermaid fences and draw each within `columns`.
 * `arriving` says the message is still streaming, so an unclosed fence draws
 * its newest complete statements; otherwise it draws whole. A fence that does
 * not draw or fit stays text.
 */
export function drawMessage(markdown: string, columns: number, arriving: boolean): Part[] {
  return scanFences(markdown).map(({ raw, mermaid }) => ({
    raw,
    indent: mermaid?.indent ?? "",
    drawing: mermaid ? draw(mermaid, columns - mermaid.indent.length, arriving) : null,
    fence: mermaid ? (mermaid.closed ? "closed" : "open") : null,
  }));
}

/** `text` as a code fence long enough to hold any backtick run inside it. */
export function fenced(text: string, info = ""): string {
  const longestRun = Math.max(0, ...Array.from(text.matchAll(/`+/g), (match) => match[0].length));
  const fence = "`".repeat(Math.max(3, longestRun + 1));
  return `${fence}${info}\n${text}\n${fence}\n`;
}

/** The art as ANSI lines, its default diff borders dim. */
export function ansiLines({ art, dimStrokes }: Drawn): string[] {
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
