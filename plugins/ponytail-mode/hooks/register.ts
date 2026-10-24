import type { PromptSubmitInput, Register } from "claude-code";
import { atom, read, update } from "claude-code";
import type { Level } from "../types";

const LEVELS: readonly Level[] = ["lite", "full", "ultra", "off"];
const STOP = /\b(stop ponytail|normal mode)\b/i;

const level = atom({ plugin: "ponytail-mode", key: "level" } as const, "full" as Level);

// The rules stay in the ponytail skill. This section only points at it, so the
// skill is the one copy of the rules: the mod cannot read loom's skills/ folder
// once the marketplace installs the mod folder alone.
function section(current: Level): string {
  return [
    `Ponytail mode is on, level: ${current}.`,
    "Before your first reply to a coding task, load the `ponytail` skill with the Skill tool,",
    "and apply it to every reply for the rest of the session.",
    'Switch the level with `/ponytail lite|full|ultra`. "stop ponytail" or "normal mode" turns it off.',
    "A later message that sets a new level or turns ponytail off overrides this section.",
  ].join("\n");
}

// The engine composes the system prompt once per session (again only after a
// compaction or a resume), so a switch cannot reach the model through the
// section. The switch goes beside the prompt that made it, and the transcript
// keeps it for the rest of the session.
function note(chosen: Level): string {
  return chosen === "off"
    ? "Ponytail mode is now off. Ignore the ponytail section of the system prompt from this reply on."
    : `Ponytail mode is now on, level: ${chosen}. Load the \`ponytail\` skill if it is not loaded, and apply that level from this reply on.`;
}

function chosenLevel(e: PromptSubmitInput): Level | undefined {
  if (e.command) {
    const word = e.command.args.trim().split(/\s+/)[0]?.toLowerCase();
    return e.command.name === "ponytail" ? LEVELS.find((name) => name === word) : undefined;
  }
  return STOP.test(e.text) ? "off" : undefined;
}

export const register: Register = (on) => {
  on("prompt.submit", async ($, e, next) => {
    const chosen = chosenLevel(e);
    if (!chosen) {
      return next(e);
    }
    await update($, level, () => chosen);
    return next({ ...e, context: [...(e.context ?? []), note(chosen)] });
  });

  on("prompt.compose", async ($, e, next) => {
    const result = await next(e);
    const current = await read($, level);
    if (current === "off") {
      return result;
    }
    return {
      sections: [
        ...result.sections,
        { id: "ponytail-mode:rules", text: section(current), scope: "session" as const },
      ],
    };
  });
};
