import { mkdir, readFile, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join } from "node:path";

export const DEFAULT_SUPPORTED_MODELS = ["openai/*", "openai-codex/*", "xai/*"] as const;

export type FooterMode = "replace" | "status" | "off";
export type FastColorValue = string | number;

export interface FastFooterConfig {
  mode: FooterMode;
  vars: Record<string, string>;
  darkFastColor?: FastColorValue | undefined;
  lightFastColor?: FastColorValue | undefined;
}

export interface FastConfig {
  persistState: boolean;
  desiredActive: boolean;
  supportedModels: string[];
  footer: FastFooterConfig;
}

export const DEFAULT_FAST_CONFIG: FastConfig = {
  persistState: false,
  desiredActive: false,
  supportedModels: [...DEFAULT_SUPPORTED_MODELS],
  footer: { mode: "replace", vars: {} },
};

export function defaultFastConfig(): FastConfig {
  return {
    ...DEFAULT_FAST_CONFIG,
    supportedModels: [...DEFAULT_FAST_CONFIG.supportedModels],
    footer: { ...DEFAULT_FAST_CONFIG.footer, vars: {} },
  };
}

export interface FastConfigResult {
  config: FastConfig;
  warnings: string[];
}

// Colors accept hex, a 256-color index, a name from footer.vars, or "" for the terminal default.

const HEX_COLOR = /^#[0-9a-fA-F]{6}$/;
const INTEGER_INDEX = /^\d+$/;
const COLOR_VAR = /^[A-Za-z_][A-Za-z0-9_.-]*$/;

export function normalizeFastColorValue(value: unknown): FastColorValue | undefined {
  if (typeof value === "number") {
    return Number.isInteger(value) && value >= 0 && value <= 255 ? value : undefined;
  }
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  if (trimmed === "" || HEX_COLOR.test(trimmed)) return trimmed;
  if (INTEGER_INDEX.test(trimmed)) {
    return Number(trimmed) <= 255 ? trimmed : undefined;
  }
  return COLOR_VAR.test(trimmed) ? trimmed : undefined;
}

/** Follows color variables until it finds a color, a missing name, or a loop. */
export function resolveFastColorValue(
  value: FastColorValue,
  vars: Readonly<Record<string, string>>,
  visited = new Set<string>(),
): { value: FastColorValue } | { error: string } {
  if (
    typeof value === "number" ||
    value === "" ||
    HEX_COLOR.test(value) ||
    INTEGER_INDEX.test(value)
  ) {
    return { value };
  }
  if (visited.has(value)) {
    return { error: `color variable ${JSON.stringify(value)} refers back to itself` };
  }
  visited.add(value);
  const referenced = Object.hasOwn(vars, value) ? vars[value] : undefined;
  if (typeof referenced !== "string") {
    return { error: `define color variable ${JSON.stringify(value)} in footer.vars` };
  }
  const normalized = normalizeFastColorValue(referenced);
  if (normalized === undefined) {
    return {
      error: "use a hex color (#rrggbb), an index (0–255), a color variable, or an empty string",
    };
  }
  return resolveFastColorValue(normalized, vars, visited);
}

type JsonRecord = Record<string, unknown>;

function isRecord(value: unknown): value is JsonRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isFooterMode(value: unknown): value is FooterMode {
  return value === "replace" || value === "status" || value === "off";
}

function describe(value: unknown): string {
  return JSON.stringify(value) ?? String(value);
}

function validModelKey(value: unknown): string | undefined {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  const slash = trimmed.indexOf("/");
  const invalid =
    trimmed.length === 0 ||
    /\s/.test(trimmed) ||
    slash <= 0 ||
    slash === trimmed.length - 1 ||
    /[*[\](){}?+|^$\\]/.test(trimmed.slice(slash + 1) === "*" ? trimmed.slice(0, slash) : trimmed);
  return invalid ? undefined : trimmed;
}

/** Keeps valid model entries. Returns undefined if the setting is not an array. */
function readSupportedModels(
  value: unknown,
  path: string,
  warnings: string[],
): string[] | undefined {
  if (!Array.isArray(value)) {
    warnings.push(`Ignoring supportedModels in ${path}. Use an array such as ["openai/*"].`);
    return undefined;
  }
  const kept: string[] = [];
  const dropped: unknown[] = [];
  for (const entry of value) {
    const normalized = validModelKey(entry);
    if (normalized === undefined) dropped.push(entry);
    else kept.push(normalized);
  }
  if (dropped.length > 0) {
    warnings.push(
      `Ignoring these supportedModels entries in ${path}: ${dropped.map(describe).join(", ")}. Use provider/model or provider/*.`,
    );
  }
  if (value.length > 0 && kept.length === 0) {
    warnings.push(
      `None of the supportedModels entries in ${path} are valid. This list allows no models.`,
    );
  }
  return kept;
}

function readColor(
  value: unknown,
  vars: Readonly<Record<string, string>>,
  field: string,
  path: string,
  warnings: string[],
): FastColorValue | undefined {
  const normalized = normalizeFastColorValue(value);
  const resolution =
    normalized === undefined
      ? {
          error:
            "use a hex color (#rrggbb), an index (0–255), a color variable, or an empty string",
        }
      : resolveFastColorValue(normalized, vars);
  if ("error" in resolution) {
    warnings.push(`Ignoring ${field}=${describe(value)} in ${path}: ${resolution.error}.`);
    return undefined;
  }
  return normalized;
}

function stringEntries(source: JsonRecord): Record<string, string> {
  return Object.fromEntries(
    Object.entries(source).filter(
      (entry): entry is [string, string] => typeof entry[1] === "string",
    ),
  );
}

function mergeConfig(
  base: FastConfig,
  source: JsonRecord,
  path: string,
  warnings: string[],
): FastConfig {
  const next: FastConfig = {
    ...base,
    footer: { ...base.footer },
  };
  if (typeof source.persistState === "boolean") next.persistState = source.persistState;
  if (typeof source.desiredActive === "boolean") next.desiredActive = source.desiredActive;
  if (Object.hasOwn(source, "supportedModels")) {
    const models = readSupportedModels(source.supportedModels, path, warnings);
    if (models !== undefined) next.supportedModels = models;
  }
  if (isRecord(source.footer)) mergeFooterConfig(next.footer, source.footer, path, warnings);
  return next;
}

function mergeFooterConfig(
  target: FastFooterConfig,
  source: JsonRecord,
  path: string,
  warnings: string[],
): void {
  if (isFooterMode(source.mode)) target.mode = source.mode;
  if (isRecord(source.vars)) target.vars = stringEntries(source.vars);
  for (const [key, field] of [
    ["darkFastColor", "footer.darkFastColor"],
    ["lightFastColor", "footer.lightFastColor"],
  ] as const) {
    if (Object.hasOwn(source, key)) {
      const color = readColor(source[key], target.vars, field, path, warnings);
      if (color !== undefined) target[key] = color;
    }
  }
}

function sanitizeFooterRecord(source: JsonRecord, path: string, warnings: string[]): JsonRecord {
  const footer: JsonRecord = { ...source };
  const vars = isRecord(source.vars) ? stringEntries(source.vars) : {};
  if (Object.hasOwn(footer, "mode") && !isFooterMode(footer.mode)) delete footer.mode;
  if (Object.hasOwn(footer, "vars")) {
    if (isRecord(source.vars)) footer.vars = vars;
    else delete footer.vars;
  }
  for (const [key, field] of [
    ["darkFastColor", "footer.darkFastColor"],
    ["lightFastColor", "footer.lightFastColor"],
  ] as const) {
    if (Object.hasOwn(footer, key)) {
      const color = readColor(footer[key], vars, field, path, warnings);
      if (color === undefined) delete footer[key];
      else footer[key] = color;
    }
  }
  return footer;
}

/** Removes invalid settings before saving and leaves unknown fields unchanged. */
function sanitizeRecord(source: JsonRecord, path: string, warnings: string[]): JsonRecord {
  const next: JsonRecord = { ...source };
  for (const field of ["persistState", "desiredActive"]) {
    if (Object.hasOwn(next, field) && typeof next[field] !== "boolean") delete next[field];
  }
  if (Object.hasOwn(next, "supportedModels")) {
    const models = readSupportedModels(next.supportedModels, path, warnings);
    if (models === undefined) delete next.supportedModels;
    else next.supportedModels = models;
  }
  if (Object.hasOwn(next, "footer")) {
    if (isRecord(next.footer)) next.footer = sanitizeFooterRecord(next.footer, path, warnings);
    else delete next.footer;
  }
  return next;
}

export function configPaths(cwd: string): { project: string; global: string } {
  return {
    project: join(cwd, ".pi", "extensions", "pi-fast.json"),
    global: join(homedir(), ".pi", "agent", "extensions", "pi-fast.json"),
  };
}

type ReadResult = { kind: "missing" } | { kind: "failed" } | { kind: "loaded"; record: JsonRecord };

async function readRecord(path: string): Promise<ReadResult> {
  let text: string;
  try {
    text = await readFile(path, "utf8");
  } catch (error) {
    if (isRecord(error) && (error.code === "ENOENT" || error.code === "ENOTDIR")) {
      return { kind: "missing" };
    }
    return { kind: "failed" };
  }
  try {
    const parsed: unknown = JSON.parse(text);
    if (isRecord(parsed)) return { kind: "loaded", record: parsed };
  } catch {
    // Invalid JSON is handled like an unreadable file.
  }
  return { kind: "failed" };
}

async function writeRecord(path: string, record: object, warnings: string[]): Promise<boolean> {
  try {
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, `${JSON.stringify(record, null, 2)}\n`, "utf8");
    return true;
  } catch {
    warnings.push(
      `Couldn't save settings to ${path}. Check that the file and its directory are writable.`,
    );
    return false;
  }
}

/** Loads global settings, then project overrides. Creates global defaults if neither file exists. */
export async function loadConfig(cwd: string): Promise<FastConfigResult> {
  const paths = configPaths(cwd);
  const warnings: string[] = [];
  let config = defaultFastConfig();

  const [globalLayer, projectLayer] = await Promise.all([
    readRecord(paths.global),
    readRecord(paths.project),
  ]);

  if (globalLayer.kind === "missing" && projectLayer.kind === "missing") {
    await writeRecord(paths.global, config, warnings);
    return { config, warnings };
  }
  for (const [layer, path] of [
    [globalLayer, paths.global],
    [projectLayer, paths.project],
  ] as const) {
    if (layer.kind === "failed") {
      warnings.push(
        `Couldn't load ${path}. Check that it is readable and contains a JSON object. Keeping other settings and defaults.`,
      );
    } else if (layer.kind === "loaded") {
      config = mergeConfig(config, layer.record, path, warnings);
    }
  }
  return { config, warnings };
}

/** Saves the /fast choice to the project settings file if it exists, or to global settings otherwise. */
export async function saveDesiredActive(
  cwd: string,
  desiredActive: boolean,
): Promise<{ ok: boolean; warnings: string[] }> {
  const paths = configPaths(cwd);
  const warnings: string[] = [];
  const projectRead = await readRecord(paths.project);
  const target = projectRead.kind === "missing" ? paths.global : paths.project;
  const existing = projectRead.kind === "missing" ? await readRecord(paths.global) : projectRead;

  if (existing.kind === "failed") {
    warnings.push(
      `Couldn't save your Fast Mode choice to ${target}. Check that the file is readable and contains a JSON object. The file was left unchanged.`,
    );
    return { ok: false, warnings };
  }
  const record =
    existing.kind === "loaded"
      ? sanitizeRecord(existing.record, target, warnings)
      : defaultFastConfig();
  record.desiredActive = desiredActive;
  const ok = await writeRecord(target, record, warnings);
  return { ok, warnings };
}
