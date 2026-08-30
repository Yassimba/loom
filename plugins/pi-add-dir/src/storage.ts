import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { CONFIG_DIR_NAME, getAgentDir } from "@earendil-works/pi-coding-agent";

export type DirectoryScope = "session" | "project" | "global";

export const SCOPE_LABELS: Record<DirectoryScope, string> = {
  session: "This session",
  project: "This project",
  global: "All projects (global)",
};

interface DirectoryConfig {
  directories: string[];
}

type ConfigRead =
  | { kind: "missing" }
  | { kind: "loaded"; directories: string[] }
  | { kind: "failed"; error: string };

function readConfig(path: string): ConfigRead {
  try {
    const parsed = JSON.parse(readFileSync(path, "utf8")) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return { kind: "failed", error: "expected a JSON object" };
    }
    const directories = (parsed as Partial<DirectoryConfig>).directories;
    return {
      kind: "loaded",
      directories: Array.isArray(directories)
        ? directories.filter((directory): directory is string => typeof directory === "string")
        : [],
    };
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
): { directories: string[]; warning?: string } {
  const path = directoryConfigPath(scope, cwd);
  const result = readConfig(path);
  if (result.kind === "loaded") return { directories: result.directories };
  if (result.kind === "missing") return { directories: [] };
  return { directories: [], warning: `Could not read ${path}: ${result.error}` };
}

export function saveDirectories(
  scope: Exclude<DirectoryScope, "session">,
  cwd: string,
  directories: string[],
): { ok: true } | { ok: false; error: string } {
  const path = directoryConfigPath(scope, cwd);
  const existing = readConfig(path);
  if (existing.kind === "failed") {
    return { ok: false, error: `Could not write ${path}: ${existing.error}` };
  }
  try {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, `${JSON.stringify({ directories }, null, 2)}\n`, "utf8");
    return { ok: true };
  } catch (error) {
    return { ok: false, error: `Could not write ${path}: ${(error as Error).message}` };
  }
}
