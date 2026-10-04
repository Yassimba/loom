/** What Pi and the Claude Code mod both do around the renderer. */

/** The system prompt note that makes the model reach for Mermaid and the diff markers. */
export const GUIDANCE = `Use fenced \`mermaid\` blocks; they render automatically in the user’s session. Always use Mermaid when it is easier to read than prose. Choose by subject: architecture for deployed services, flowchart for dependencies/decisions, sequence for interactions, state for lifecycles, ER/class for models, mindmap for hierarchies, timeline/git graph for history, pie for proportions. Use complementary diagrams when explaining multiple aspects. Keep diagram labels short. Mark changes by appending :::red (removed), :::green (added), or :::orange (changed) after the node, outside its label brackets (A[Added]:::green, never A[Added :::green]); their default colors are automatic—do not add classDef for these diff markers. In general prefer colored outlines to logically group things (if there are no changes involved).`;

const diffClasses = {
  red: { stroke: "#9f5555", definition: "classDef red stroke:#9f5555", sgr: "38;2;159;85;85" },
  orange: {
    stroke: "#9a7438",
    definition: "classDef orange stroke:#9a7438",
    sgr: "38;2;154;116;56",
  },
  green: { stroke: "#4f8560", definition: "classDef green stroke:#4f8560", sgr: "38;2;79;133;96" },
};

/** Give the diff markers their default colors; `dimSgr` and `dimStrokes` name the ones drawn dim. */
export function withDiffClasses(source: string): {
  source: string;
  dimSgr: string[];
  dimStrokes: string[];
} {
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
    dimStrokes: defaults.map(([, style]) => style.stroke),
  };
}
