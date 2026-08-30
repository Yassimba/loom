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
import { type DirectoryScope, loadDirectories, SCOPE_LABELS, saveDirectories } from "./storage.ts";

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

async function resolveDirectoryInput(
  args: string,
  ctx: ExtensionCommandContext,
): Promise<string | undefined> {
  const input = args.trim() || (await ctx.ui.input("Directory to add:", "../"))?.trim();
  if (!input) return undefined;
  if (input.includes("\n") || input.includes("\r")) {
    ctx.ui.notify("Directory paths cannot contain newlines", "error");
    return undefined;
  }
  const directory = absoluteDirectory(input, ctx.cwd);
  if (!directory) {
    ctx.ui.notify(`Not a directory: ${input}`, "error");
    return undefined;
  }
  if (directory === absoluteDirectory(ctx.cwd, ctx.cwd)) {
    ctx.ui.notify("The current working directory is already in context", "info");
    return undefined;
  }
  return directory;
}

async function selectDirectoryScope(
  directory: string,
  ctx: ExtensionCommandContext,
): Promise<DirectoryScope | undefined> {
  const available: DirectoryScope[] = ctx.isProjectTrusted()
    ? ["session", "project", "global"]
    : ["session", "global"];
  const label = ctx.hasUI
    ? await ctx.ui.select(
        "Add directory for:",
        available.map((scope) => SCOPE_LABELS[scope]),
      )
    : SCOPE_LABELS.session;
  const scope = available.find((candidate) => SCOPE_LABELS[candidate] === label);
  if (scope !== "global") return scope;
  const confirmed = await ctx.ui.confirm(
    "Add directory globally?",
    `This directory and its AGENTS.md/CLAUDE.md instructions will be added to every project.\n\n${directory}`,
  );
  return confirmed ? scope : undefined;
}

export default function piAddDir(pi: ExtensionAPI): void {
  let sessionDirectories: string[] = [];
  let projectDirectories: string[] = [];
  let globalDirectories: string[] = [];
  let favoriteDirectories: string[] = [];
  let sessionCwd = process.cwd();

  function directoriesFor(scope: DirectoryScope): string[] {
    if (scope === "session") return sessionDirectories;
    return scope === "project" ? projectDirectories : globalDirectories;
  }

  function allDirectories(): string[] {
    return [...new Set([...sessionDirectories, ...projectDirectories, ...globalDirectories])];
  }

  function updateStatus(ctx: ExtensionContext): void {
    ctx.ui.setStatus(STATUS_KEY, footerStatus(allDirectories()));
  }

  function persistSession(ctx: ExtensionContext): void {
    pi.appendEntry(STATE_ENTRY, { directories: sessionDirectories });
    updateStatus(ctx);
  }

  function replaceDirectories(scope: DirectoryScope, directories: string[]): void {
    if (scope === "session") sessionDirectories = directories;
    else if (scope === "project") projectDirectories = directories;
    else globalDirectories = directories;
  }

  function persistScope(
    scope: DirectoryScope,
    directories: string[],
    ctx: ExtensionContext,
  ): boolean {
    if (scope === "session") {
      sessionDirectories = directories;
      persistSession(ctx);
      return true;
    }
    const result = saveDirectories(scope, ctx.cwd, directories);
    if (!result.ok) {
      ctx.ui.notify(result.error, "error");
      return false;
    }
    replaceDirectories(scope, directories);
    updateStatus(ctx);
    return true;
  }

  pi.on("session_start", async (_event, ctx) => {
    sessionCwd = ctx.cwd;
    sessionDirectories = restoreDirectories(ctx);
    const global = loadDirectories("global", ctx.cwd);
    const project = ctx.isProjectTrusted()
      ? loadDirectories("project", ctx.cwd)
      : { directories: [], warning: undefined };
    globalDirectories = global.directories;
    projectDirectories = project.directories;
    for (const warning of [global.warning, project.warning]) {
      if (warning) ctx.ui.notify(warning, "warning");
    }
    updateStatus(ctx);

    ctx.ui.addAutocompleteProvider((current) => ({
      triggerCharacters: current.triggerCharacters,
      async getSuggestions(lines, cursorLine, cursorCol, options) {
        const parsed = parseDirCommand((lines[cursorLine] ?? "").slice(0, cursorCol));
        if (!parsed) return current.getSuggestions(lines, cursorLine, cursorCol, options);
        const items =
          parsed.command === "add-dir"
            ? completeDirectories(parsed.prefix, ctx.cwd, favoriteDirectories)
            : completeAddedDirectories(parsed.prefix, allDirectories());
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
    const directories = allDirectories();
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
      const directory = await resolveDirectoryInput(args, ctx);
      if (!directory) return;
      const scope = await selectDirectoryScope(directory, ctx);
      if (!scope) return;
      const scopedDirectories = directoriesFor(scope);
      if (scopedDirectories.includes(directory)) {
        ctx.ui.notify(
          `${directory} is already added for ${SCOPE_LABELS[scope].toLowerCase()}`,
          "info",
        );
        return;
      }
      if (!persistScope(scope, [...scopedDirectories, directory], ctx)) return;
      ctx.ui.notify(`Added ${directory} for ${SCOPE_LABELS[scope].toLowerCase()}`, "info");
    },
  });

  pi.registerCommand("rm-dir", {
    description: "Remove a directory from the agent context",
    getArgumentCompletions(prefix) {
      return completeAddedDirectories(prefix, allDirectories());
    },
    handler: async (args: string, ctx: ExtensionCommandContext) => {
      const directories = allDirectories();
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
      const scopes = (["session", "project", "global"] as const).filter((scope) =>
        directoriesFor(scope).includes(match),
      );
      let scope: DirectoryScope | undefined = scopes[0];
      if (scopes.length > 1) {
        const selectedLabel = await ctx.ui.select(
          "Remove directory from:",
          scopes.map((candidate) => SCOPE_LABELS[candidate]),
        );
        scope = scopes.find((candidate) => SCOPE_LABELS[candidate] === selectedLabel);
      }
      if (!scope) return;
      const remaining = directoriesFor(scope).filter((directory) => directory !== match);
      if (!persistScope(scope, remaining, ctx)) return;
      ctx.ui.notify(`Removed ${match} from ${SCOPE_LABELS[scope].toLowerCase()}`, "info");
    },
  });

  pi.registerCommand("dirs", {
    description: "List directories added by /add-dir",
    handler: async (_args, ctx) => {
      const lines = (["session", "project", "global"] as const).flatMap((scope) =>
        directoriesFor(scope).map((directory) => `${SCOPE_LABELS[scope]}: ${directory}`),
      );
      if (lines.length === 0) {
        ctx.ui.notify("No external directories", "info");
        return;
      }
      ctx.ui.notify(lines.join("\n"), "info");
    },
  });
}
