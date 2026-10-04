/** What Pi and the Claude Code mod both do around the renderer. */

import { render } from "./loom-mermaid/index.ts";
import type { MermaidArt } from "./loom-mermaid/types.ts";

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
export function drawDiagram(source: string, columns: number): Drawn | null {
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
