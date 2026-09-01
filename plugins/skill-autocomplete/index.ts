import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import {
  type AutocompleteItem,
  type AutocompleteProvider,
  fuzzyFilter,
} from "@earendil-works/pi-tui";

type SkillCommand = ReturnType<ExtensionAPI["getCommands"]>[number];

export function extractSkillQuery(textBeforeCursor: string): string | undefined {
  return textBeforeCursor.match(/\$([^\s$]*)$/)?.[1];
}

export function skillItems(commands: SkillCommand[], query: string): AutocompleteItem[] {
  const skills = commands.filter((command) => command.source === "skill");
  return fuzzyFilter(
    skills,
    query,
    (command) => `${command.name.slice(6)} ${command.description ?? ""}`,
  ).map((command) => ({
    value: `$${command.name.slice(6)}`,
    label: `$${command.name.slice(6)}`,
    description: command.description,
  }));
}

export default function piSkillAutocomplete(pi: ExtensionAPI): void {
  pi.on("session_start", (_event, ctx) => {
    ctx.ui.addAutocompleteProvider(
      (current): AutocompleteProvider => ({
        triggerCharacters: ["$"],
        async getSuggestions(lines, cursorLine, cursorCol, options) {
          const query = extractSkillQuery((lines[cursorLine] ?? "").slice(0, cursorCol));
          if (query === undefined) {
            return current.getSuggestions(lines, cursorLine, cursorCol, options);
          }

          const items = skillItems(pi.getCommands(), query);
          return items.length > 0 ? { items, prefix: `$${query}` } : null;
        },
        applyCompletion(lines, cursorLine, cursorCol, item, prefix) {
          return current.applyCompletion(lines, cursorLine, cursorCol, item, prefix);
        },
        shouldTriggerFileCompletion(lines, cursorLine, cursorCol) {
          return current.shouldTriggerFileCompletion?.(lines, cursorLine, cursorCol) ?? true;
        },
      }),
    );
  });
}
