import { existsSync, readdirSync, readFileSync, realpathSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { basename, isAbsolute, join, resolve, sep } from "node:path";
import {
  CustomEditor,
  type ExtensionAPI,
  type ExtensionCommandContext,
  type ExtensionContext,
} from "@earendil-works/pi-coding-agent";
import { type AutocompleteItem, Key, matchesKey } from "@earendil-works/pi-tui";

type Access = "read" | "write";
type Scope = "session" | "project" | "global";

interface AddedDirectory {
  path: string;
  access?: Access;
  scope?: Scope;
}

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

export class AddDirEditor extends CustomEditor {
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

const STATE_ENTRY = "pi-add-dir-prototype:state";
const SANDBOX_GRANT_EVENT = "pi-sandbox:grant-path";
const CONTEXT_FILES = ["AGENTS.md", "CLAUDE.md"] as const;

export const PERMISSION_CHOICES = [
  { label: "Read only · Session", access: "read", scope: "session" },
  { label: "Read + write · Session", access: "write", scope: "session" },
  { label: "Read only · Project", access: "read", scope: "project" },
  { label: "Read + write · Project", access: "write", scope: "project" },
  { label: "Read only · Global", access: "read", scope: "global" },
  { label: "Read + write · Global", access: "write", scope: "global" },
] as const;

function expandHome(input: string): string {
  if (input === "~") return homedir();
  if (input.startsWith(`~${sep}`)) return join(homedir(), input.slice(2));
  return input;
}

function absoluteDirectory(input: string, cwd: string): string | undefined {
  const expanded = expandHome(input.trim());
  const candidate = isAbsolute(expanded) ? expanded : resolve(cwd, expanded);
  try {
    return statSync(candidate).isDirectory() ? realpathSync(candidate) : undefined;
  } catch {
    return undefined;
  }
}

function splitCompletionPrefix(prefix: string): { parentText: string; fragment: string } {
  const slash = Math.max(prefix.lastIndexOf("/"), prefix.lastIndexOf("\\"));
  if (slash < 0) return { parentText: "", fragment: prefix };
  return { parentText: prefix.slice(0, slash + 1), fragment: prefix.slice(slash + 1) };
}

export function completeDirectories(prefix: string, cwd: string, favorites: string[] = []) {
  const finalSegment = splitCompletionPrefix(prefix).fragment;
  const completionPrefix =
    finalSegment === "." || finalSegment === ".." ? `${prefix}${sep}` : prefix;
  const { parentText, fragment } = splitCompletionPrefix(completionPrefix);
  const parent = absoluteDirectory(parentText || ".", cwd);
  if (!parent) return null;

  try {
    const rankedFavorites = favorites
      .map((directory) => absoluteDirectory(directory, cwd))
      .filter((directory): directory is string => directory !== undefined);
    const favoriteRanks = new Map(rankedFavorites.map((directory, rank) => [directory, rank]));
    const localItems = readdirSync(parent, { withFileTypes: true })
      .filter((entry) => entry.isDirectory() && entry.name.startsWith(fragment))
      .map((entry) => {
        const value = `${parentText}${entry.name}${sep}`;
        const absolutePath = absoluteDirectory(value, cwd);
        const rank = absolutePath === undefined ? undefined : favoriteRanks.get(absolutePath);
        return {
          value,
          label: `${rank === undefined ? "" : "★ "}${entry.name}${sep}`,
          description: rank === undefined ? value : `zoxide #${rank + 1} · ${absolutePath}`,
          rank,
        };
      })
      .sort(
        (left, right) =>
          (left.rank ?? Number.POSITIVE_INFINITY) - (right.rank ?? Number.POSITIVE_INFINITY) ||
          left.label.localeCompare(right.label),
      )
      .map(({ rank: _rank, ...item }) => item);

    if (prefix !== "") return localItems.length > 0 ? localItems : null;

    const localPaths = new Set(
      localItems.map((item) => absoluteDirectory(item.value, cwd)).filter(Boolean),
    );
    const favoriteItems = rankedFavorites
      .filter((directory) => !localPaths.has(directory))
      .slice(0, 8)
      .map((directory) => ({
        value: `${directory}${sep}`,
        label: `★ ${basename(directory)}${sep}`,
        description: `zoxide · ${directory}`,
      }));
    const items = [...favoriteItems, ...localItems];
    return items.length > 0 ? items : null;
  } catch {
    return null;
  }
}

async function grantSandboxPath(
  pi: ExtensionAPI,
  ctx: ExtensionCommandContext,
  path: string,
  access: Access,
  scope: Scope,
): Promise<void> {
  let accepted = false;
  await new Promise<void>((resolve, reject) => {
    pi.events.emit(SANDBOX_GRANT_EVENT, {
      access,
      path,
      scope,
      ctx,
      accept: () => {
        accepted = true;
      },
      resolve,
      reject,
    });
    if (!accepted) reject(new Error("The pi-sandbox bridge is not installed"));
  });
}

function readContext(directory: string): string {
  const sections: string[] = [];
  for (const filename of CONTEXT_FILES) {
    const path = join(directory, filename);
    if (!existsSync(path)) continue;
    try {
      sections.push(`### ${filename} from ${directory}\n\n${readFileSync(path, "utf8")}`);
    } catch {
      // An unreadable context file should not hide the directory itself.
    }
  }
  return sections.join("\n\n");
}

function restoreDirectories(ctx: ExtensionContext): AddedDirectory[] {
  let directories: AddedDirectory[] = [];
  for (const entry of ctx.sessionManager.getBranch()) {
    if (entry.type === "custom" && entry.customType === STATE_ENTRY) {
      directories = (entry.data as { directories?: AddedDirectory[] }).directories ?? [];
    }
  }
  return directories;
}

export default function piAddDirPrototype(pi: ExtensionAPI): void {
  let directories: AddedDirectory[] = [];
  let favoriteDirectories: string[] = [];

  function updateWidget(ctx: ExtensionContext): void {
    if (directories.length === 0) {
      ctx.ui.setWidget(STATE_ENTRY, undefined);
      return;
    }
    const labels = directories.map((directory) => basename(directory.path)).join(", ");
    ctx.ui.setWidget(STATE_ENTRY, [`📂 ${directories.length} external: ${labels}  (/dirs)`]);
  }

  pi.on("session_start", async (_event, ctx) => {
    directories = restoreDirectories(ctx);
    updateWidget(ctx);

    ctx.ui.addAutocompleteProvider((current) => ({
      triggerCharacters: current.triggerCharacters,
      async getSuggestions(lines, cursorLine, cursorCol, options) {
        const beforeCursor = (lines[cursorLine] ?? "").slice(0, cursorCol);
        const match = beforeCursor.match(/^\/add-dir\s(.*)$/);
        if (!match) {
          return current.getSuggestions(lines, cursorLine, cursorCol, options);
        }
        const prefix = match[1] ?? "";
        const items = completeDirectories(prefix, ctx.cwd, favoriteDirectories);
        return items ? { items, prefix } : null;
      },
      applyCompletion(lines, cursorLine, cursorCol, item, prefix) {
        return current.applyCompletion(lines, cursorLine, cursorCol, item, prefix);
      },
      shouldTriggerFileCompletion(lines, cursorLine, cursorCol) {
        const beforeCursor = (lines[cursorLine] ?? "").slice(0, cursorCol);
        if (/^\/add-dir\s/.test(beforeCursor)) return true;
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

  pi.on("before_agent_start", (event) => {
    if (directories.length === 0) return;
    const sections = directories.map((directory) => {
      const context = readContext(directory.path);
      const access = directory.access === "write" ? "read and write" : "read only";
      const scope = directory.scope ?? "session";
      return [
        `## External directory: ${directory.path}`,
        `Sandbox access: ${access} (${scope}). Use absolute paths for tools.`,
        context,
      ]
        .filter(Boolean)
        .join("\n\n");
    });
    return { systemPrompt: `${event.systemPrompt}\n\n${sections.join("\n\n")}` };
  });

  pi.registerCommand("add-dir", {
    description: "Add a directory to context and request pi-sandbox access",
    getArgumentCompletions(prefix) {
      return completeDirectories(prefix, process.cwd(), favoriteDirectories);
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
      const existing = directories.find((item) => item.path === directory);
      const selectedLabel = await ctx.ui.select(
        "Sandbox access and lifetime:",
        PERMISSION_CHOICES.map((choice) => choice.label),
      );
      const choice = PERMISSION_CHOICES.find((candidate) => candidate.label === selectedLabel);
      if (!choice) return;

      try {
        await grantSandboxPath(pi, ctx, directory, choice.access, choice.scope);
      } catch (error) {
        ctx.ui.notify(
          `Could not grant sandbox access: ${error instanceof Error ? error.message : error}`,
          "error",
        );
        return;
      }

      if (existing) {
        existing.access = choice.access;
        existing.scope = choice.scope;
      } else {
        directories = [
          ...directories,
          { path: directory, access: choice.access, scope: choice.scope },
        ];
      }
      pi.appendEntry(STATE_ENTRY, { directories });
      updateWidget(ctx);
      ctx.ui.notify(`Added ${directory} to context and sandbox`, "info");
    },
  });

  pi.registerCommand("dirs", {
    description: "List directories added by /add-dir",
    handler: async (_args, ctx) => {
      if (directories.length === 0) {
        ctx.ui.notify("No external directories added", "info");
        return;
      }
      ctx.ui.notify(
        directories
          .map(
            (directory) =>
              `${directory.access === "write" ? "read + write" : "read only"} · ${directory.scope ?? "session"}  ${directory.path}`,
          )
          .join("\n"),
        "info",
      );
    },
  });
}
