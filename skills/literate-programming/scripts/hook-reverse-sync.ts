#!/usr/bin/env bun
/**
 * hook-reverse-sync.ts: PostToolUse hook entry point for Claude Code.
 *
 * Reads the tool use event from stdin, checks if the edited file is in a
 * .lit.map.json manifest, and runs untangle if so.
 *
 * Does NOT weave (too slow for every edit). Weaving happens when the user
 * explicitly runs /literate-programming on an existing .lit.md.
 */

import { readFileSync, existsSync, writeFileSync, unlinkSync } from "fs";
import { resolve, dirname, join } from "path";
import { execSync } from "child_process";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface ToolEvent {
  tool_name: string;
  tool_input: {
    file_path?: string;
    command?: string;
    [key: string]: unknown;
  };
  [key: string]: unknown;
}

interface SourceMap {
  litFile: string;
  outputDir: string;
  files: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Find the .lit.map.json manifest
// ---------------------------------------------------------------------------

function findManifest(startDir: string): string | null {
  let dir = resolve(startDir);
  while (true) {
    const files = require("fs").readdirSync(dir) as string[];
    const match = files.find((f: string) => f.endsWith(".lit.map.json"));
    if (match) return join(dir, match);

    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return null;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
  // Read event from stdin
  let input = "";
  for await (const chunk of Bun.stdin.stream()) {
    input += new TextDecoder().decode(chunk);
  }

  let event: ToolEvent;
  try {
    event = JSON.parse(input);
  } catch {
    process.exit(0);
  }

  // Only care about file-editing tools
  const editTools = ["Edit", "Write", "NotebookEdit"];
  if (!editTools.includes(event.tool_name)) {
    process.exit(0);
  }

  const filePath = event.tool_input?.file_path;
  if (!filePath) {
    process.exit(0);
  }

  const cwd = process.cwd();

  // Check lockfile to prevent infinite loops
  const lockFile = join(cwd, ".lit.untangle.lock");
  if (existsSync(lockFile)) {
    process.exit(0);
  }

  // Find manifest
  const manifestPath = findManifest(cwd);
  if (!manifestPath) {
    process.exit(0);
  }

  const manifest: SourceMap = JSON.parse(readFileSync(manifestPath, "utf-8"));

  // Check if the edited file is in the manifest
  const absFile = resolve(filePath);
  const absOutputDir = resolve(dirname(manifestPath), manifest.outputDir);

  let found = false;
  for (const relFile of Object.keys(manifest.files)) {
    if (resolve(absOutputDir, relFile) === absFile) {
      found = true;
      break;
    }
  }

  if (!found) {
    process.exit(0);
  }

  // Create lockfile, run untangle, remove lockfile
  try {
    writeFileSync(lockFile, String(process.pid));
    const untanglePath = resolve(dirname(import.meta.path), "untangle.ts");
    execSync(`bun run ${untanglePath} ${filePath}`, {
      stdio: "inherit",
      cwd,
    });
  } finally {
    try {
      unlinkSync(lockFile);
    } catch {
      // lockfile already removed
    }
  }
}

main();
