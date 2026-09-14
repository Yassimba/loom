import { canonicalizeFromCwd } from "../../src/core/paths";
import { hasShellExpansion } from "../../src/core/paths/plausibility";
import { extractBashTargets } from "../../src/shared/paths";

export interface FileTarget {
  path: string;
  unresolved: boolean;
  checkPathAccess: boolean;
}

export async function targetsForTool(
  toolName: string,
  input: Record<string, unknown>,
  cwd: string,
): Promise<FileTarget[]> {
  if (["read", "write", "edit", "grep", "find", "ls"].includes(toolName)) {
    const raw = String(input.file_path ?? input.path ?? "").trim();
    return raw
      ? [{ path: await canonicalizeFromCwd(raw, cwd), unresolved: false, checkPathAccess: true }]
      : [];
  }

  if (toolName === "bash") {
    const targets = await extractBashTargets(String(input.command ?? ""), cwd);
    return Promise.all(
      targets.map(async (target) => ({
        path: await canonicalizeFromCwd(target.path, cwd),
        unresolved: hasShellExpansion(target.path),
        checkPathAccess: target.checkPathAccess,
      })),
    );
  }

  return [];
}
