import type { Register } from "claude-code";
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
  ].join("\n");
}

function levelFrom(args: string): Level | undefined {
  const word = args.trim().split(/\s+/)[0]?.toLowerCase();
  return LEVELS.find((name) => name === word);
}

export const register: Register = (on) => {
  on("prompt.submit", async ($, e, next) => {
    const chosen = e.command?.name === "ponytail" ? levelFrom(e.command.args) : undefined;
    const isStop = !e.command && STOP.test(e.text);
    if (chosen || isStop) {
      await update($, level, () => chosen ?? "off");
    }
    return next(e);
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
