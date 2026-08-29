import {
  CustomEditor,
  type ExtensionAPI,
  type ExtensionCommandContext,
  type ExtensionContext,
} from "@earendil-works/pi-coding-agent";
import { type AutocompleteItem, Key, matchesKey } from "@earendil-works/pi-tui";
import {
  absoluteDirectory,
  completeAddedDirectories,
  completeDirectories,
  footerStatus,
  matchAddedDirectory,
  parseDirCommand,
  readContext,
} from "./paths.ts";

export {
  completeAddedDirectories,
  completeDirectories,
  footerStatus,
  matchAddedDirectory,
} from "./paths.ts";

interface EditorAutocompleteInternals {
  autocompleteState: unknown;
  autocompleteList?: { getSelectedItem(): AutocompleteItem | undefined };
  cancelAutocomplete(): void;
  tryTriggerAutocomplete(): void;
}

export function shouldSubmitAddDirPath(
  data: string,
  editorText: string,
  hasAutocomplete: boolean,
): boolean {
  return hasAutocomplete && editorText.startsWith("/add-dir ") && matchesKey(data, Key.enter);
}

class AddDirEditor extends CustomEditor {
  override handleInput(data: string): void {
    const internals = this as unknown as EditorAutocompleteInternals;
    if (shouldSubmitAddDirPath(data, this.getText(), Boolean(internals.autocompleteState))) {
      internals.cancelAutocomplete();
      super.handleInput(data);
      return;
    }

    const selected = internals.autocompleteState
      ? internals.autocompleteList?.getSelectedItem()
      : undefined;
    const shouldOpenChildren =
      matchesKey(data, Key.tab) &&
      selected?.label.endsWith("/") === true &&
      this.getText().startsWith("/add-dir ");

    super.handleInput(data);
    if (shouldOpenChildren) queueMicrotask(() => internals.tryTriggerAutocomplete());
  }
}

const STATE_ENTRY = "pi-add-dir:state";
const STATUS_KEY = "pi-add-dir";

function restoreDirectories(ctx: ExtensionContext): string[] {
  let directories: string[] = [];
  for (const entry of ctx.sessionManager.getBranch()) {
    if (entry.type === "custom" && entry.customType === STATE_ENTRY) {
      const restored = (entry.data as { directories?: string[] }).directories ?? [];
      directories = restored.filter((path) => typeof path === "string");
    }
  }
  return directories;
}

export default function piAddDir(pi: ExtensionAPI): void {
  let directories: string[] = [];
  let favoriteDirectories: string[] = [];
  let sessionCwd = process.cwd();

  function updateStatus(ctx: ExtensionContext): void {
    ctx.ui.setStatus(STATUS_KEY, footerStatus(directories));
  }

  function persist(ctx: ExtensionContext): void {
    pi.appendEntry(STATE_ENTRY, { directories });
    updateStatus(ctx);
  }

  pi.on("session_start", async (_event, ctx) => {
    sessionCwd = ctx.cwd;
    directories = restoreDirectories(ctx);
    updateStatus(ctx);

    ctx.ui.addAutocompleteProvider((current) => ({
      triggerCharacters: current.triggerCharacters,
      async getSuggestions(lines, cursorLine, cursorCol, options) {
        const parsed = parseDirCommand((lines[cursorLine] ?? "").slice(0, cursorCol));
        if (!parsed) return current.getSuggestions(lines, cursorLine, cursorCol, options);
        const items =
          parsed.command === "add-dir"
            ? completeDirectories(parsed.prefix, ctx.cwd, favoriteDirectories)
            : completeAddedDirectories(parsed.prefix, directories);
        return items ? { items, prefix: parsed.prefix } : null;
      },
      applyCompletion(lines, cursorLine, cursorCol, item, prefix) {
        return current.applyCompletion(lines, cursorLine, cursorCol, item, prefix);
      },
      shouldTriggerFileCompletion(lines, cursorLine, cursorCol) {
        if (parseDirCommand((lines[cursorLine] ?? "").slice(0, cursorCol))) return true;
        return current.shouldTriggerFileCompletion?.(lines, cursorLine, cursorCol) ?? true;
      },
    }));
    ctx.ui.setEditorComponent(
      (tui, theme, keybindings) => new AddDirEditor(tui, theme, keybindings),
    );

    try {
      const result = await pi.exec("zoxide", ["query", "-l"], { timeout: 1_000 });
      favoriteDirectories = result.stdout
        .split("\n")
        .map((directory) => absoluteDirectory(directory, ctx.cwd))
        .filter((directory): directory is string => directory !== undefined);
    } catch {
      favoriteDirectories = [];
    }
  });

  pi.on("session_shutdown", (_event, ctx) => {
    ctx.ui.setStatus(STATUS_KEY, undefined);
  });

  pi.on("before_agent_start", (event) => {
    if (directories.length === 0) return;
    const sections = directories.map((directory) => {
      const context = readContext(directory);
      return [`## External directory: ${directory}`, context].filter(Boolean).join("\n\n");
    });
    return { systemPrompt: `${event.systemPrompt}\n\n${sections.join("\n\n")}` };
  });

  pi.registerCommand("add-dir", {
    description: "Add a directory to the agent context",
    getArgumentCompletions(prefix) {
      return completeDirectories(prefix, sessionCwd, favoriteDirectories);
    },
    handler: async (args: string, ctx: ExtensionCommandContext) => {
      let input = args.trim();
      if (!input) {
        input = (await ctx.ui.input("Directory to add:", "../"))?.trim() ?? "";
      }
      if (!input) return;
      if (input.includes("\n") || input.includes("\r")) {
        ctx.ui.notify("Directory paths cannot contain newlines", "error");
        return;
      }

      const directory = absoluteDirectory(input, ctx.cwd);
      if (!directory) {
        ctx.ui.notify(`Not a directory: ${input}`, "error");
        return;
      }
      if (directory === absoluteDirectory(ctx.cwd, ctx.cwd)) {
        ctx.ui.notify("The current working directory is already in context", "info");
        return;
      }
      if (directories.includes(directory)) {
        ctx.ui.notify(`${directory} is already in context`, "info");
        return;
      }

      directories = [...directories, directory];
      persist(ctx);
      ctx.ui.notify(`Added ${directory}`, "info");
    },
  });

  pi.registerCommand("rm-dir", {
    description: "Remove a directory from the agent context",
    getArgumentCompletions(prefix) {
      return completeAddedDirectories(prefix, directories);
    },
    handler: async (args: string, ctx: ExtensionCommandContext) => {
      if (directories.length === 0) {
        ctx.ui.notify("No external directories", "info");
        return;
      }
      let input = args.trim();
      if (!input) {
        input = (await ctx.ui.select("Remove directory:", directories)) ?? "";
      }
      if (!input) return;
      const match = matchAddedDirectory(directories, input, ctx.cwd);
      if (!match) {
        ctx.ui.notify(`Not an added directory: ${input}`, "error");
        return;
      }
      directories = directories.filter((directory) => directory !== match);
      persist(ctx);
      ctx.ui.notify(`Removed ${match}`, "info");
    },
  });

  pi.registerCommand("dirs", {
    description: "List directories added by /add-dir",
    handler: async (_args, ctx) => {
      if (directories.length === 0) {
        ctx.ui.notify("No external directories", "info");
        return;
      }
      ctx.ui.notify(directories.join("\n"), "info");
    },
  });
}
