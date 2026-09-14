import { existsSync, readdirSync, readFileSync, realpathSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { basename, isAbsolute, join, resolve, sep } from "node:path";

const INSTRUCTION_FILES = ["AGENTS.md", "CLAUDE.md"] as const;
const ORIENTATION_FILES = ["README.md", "package.json"] as const;
const MAX_ORIENTATION_SOURCE_LENGTH = 12_000;
const MAX_ORIENTATION_LENGTH = 400;

export function footerStatus(directories: string[]): string | undefined {
  if (directories.length === 0) return undefined;
  return `added dirs ${directories.map((directory) => basename(directory)).join(", ")}`;
}

function expandHome(input: string): string {
  if (input === "~") return homedir();
  if (input.startsWith(`~${sep}`)) return join(homedir(), input.slice(2));
  return input;
}

export function absoluteDirectory(input: string, cwd: string): string | undefined {
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
          label: `${entry.name}${sep}`,
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
        label: `${basename(directory)}${sep}`,
        description: `zoxide · ${directory}`,
      }));
    const items = [...favoriteItems, ...localItems];
    return items.length > 0 ? items : null;
  } catch {
    return null;
  }
}

export function completeAddedDirectories(prefix: string, directories: string[]) {
  const items = directories
    .filter((directory) => directory.startsWith(prefix) || basename(directory).startsWith(prefix))
    .map((directory) => ({
      value: directory,
      label: basename(directory),
      description: directory,
    }));
  return items.length > 0 ? items : null;
}

export function matchAddedDirectory(
  directories: string[],
  input: string,
  cwd: string,
): string | undefined {
  const trimmed = input.trim();
  if (!trimmed) return undefined;
  const absolute = absoluteDirectory(trimmed, cwd);
  if (absolute && directories.includes(absolute)) return absolute;
  const matches = directories.filter(
    (directory) => directory === trimmed || basename(directory) === trimmed,
  );
  return matches.length === 1 ? matches[0] : undefined;
}

export function readOrientationSource(directory: string): string {
  const sections: string[] = [];
  for (const filename of ORIENTATION_FILES) {
    try {
      sections.push(`${filename}:\n${readFileSync(join(directory, filename), "utf8")}`);
    } catch {}
  }
  return sections.join("\n\n").slice(0, MAX_ORIENTATION_SOURCE_LENGTH);
}

export function normalizeOrientation(value: string): string | undefined {
  const orientation = value.replaceAll(/\s+/g, " ").trim();
  return orientation ? orientation.slice(0, MAX_ORIENTATION_LENGTH) : undefined;
}

export function externalDirectoryContext(directory: string, orientation: string): string {
  const instructionFiles = INSTRUCTION_FILES.filter((filename) =>
    existsSync(join(directory, filename)),
  );
  const instructions =
    instructionFiles.length > 0
      ? `Instructions available: ${instructionFiles.join(", ")}. Read and follow them before operating in this directory.`
      : undefined;
  return [`## External directory: ${directory}`, `Orientation: ${orientation}`, instructions]
    .filter(Boolean)
    .join("\n\n");
}

export function parseDirCommand(
  beforeCursor: string,
): { command: "add-dir" | "rm-dir"; prefix: string } | null {
  const match = beforeCursor.match(/^\/(add-dir|rm-dir)\s(.*)$/);
  if (!match) return null;
  return { command: match[1] as "add-dir" | "rm-dir", prefix: match[2] ?? "" };
}
