import { readFileSync } from "node:fs";
import { join } from "node:path";
import { CONFIG_DIR_NAME, getAgentDir } from "@earendil-works/pi-coding-agent";

export type DirectoryScope = "session" | "project" | "global";

export const SCOPE_LABELS: Record<DirectoryScope, string> = {
  session: "This session",
  project: "This project",
  global: "All projects (global)",
};

export interface DirectoryConfig {
  directories: string[];
  orientations: Record<string, string>;
}

type ConfigRead =
  | { kind: "missing" }
  | { kind: "loaded"; config: DirectoryConfig }
  | { kind: "failed"; error: string };

function readConfig(path: string): ConfigRead {
  try {
    const parsed = JSON.parse(readFileSync(path, "utf8")) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return { kind: "failed", error: "expected a JSON object" };
    }
    const input = parsed as Partial<DirectoryConfig>;
    const directories = Array.isArray(input.directories)
      ? input.directories.filter((directory): directory is string => typeof directory === "string")
      : [];
    const orientations =
      input.orientations && typeof input.orientations === "object"
        ? Object.fromEntries(
            Object.entries(input.orientations).filter(
              (entry): entry is [string, string] => typeof entry[1] === "string",
            ),
          )
        : {};
    return { kind: "loaded", config: { directories, orientations } };
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return { kind: "missing" };
    return { kind: "failed", error: (error as Error).message };
  }
}

export function directoryConfigPath(
  scope: Exclude<DirectoryScope, "session">,
  cwd: string,
): string {
  return scope === "project"
    ? join(cwd, CONFIG_DIR_NAME, "add-dir.json")
    : join(getAgentDir(), "add-dir.json");
}

export function loadDirectories(
  scope: Exclude<DirectoryScope, "session">,
  cwd: string,
): DirectoryConfig & { warning?: string } {
  const path = directoryConfigPath(scope, cwd);
  const result = readConfig(path);
  if (result.kind === "loaded") return result.config;
  if (result.kind === "missing") return { directories: [], orientations: {} };
  return {
    directories: [],
    orientations: {},
    warning: `Could not read ${path}: ${result.error}`,
  };
}
