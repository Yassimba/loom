import type { CodePeekProps } from "../authoring/core";
import type { SourceEvidenceResolver } from "./model";

export interface ChangedLines {
  deleted: ReadonlySet<number>;
  added: ReadonlySet<number>;
}

export function patchChangedLines(patch: string): ChangedLines {
  const deleted = new Set<number>();
  const added = new Set<number>();
  let oldLine = 0;
  let newLine = 0;
  for (const line of patch.split("\n")) {
    const hunk = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line);
    if (hunk) {
      oldLine = Number(hunk[1]);
      newLine = Number(hunk[2]);
    } else if (oldLine || newLine) {
      if (line.startsWith("-")) deleted.add(oldLine++);
      else if (line.startsWith("+")) added.add(newLine++);
      else if (line.startsWith(" ")) {
        oldLine += 1;
        newLine += 1;
      }
    }
  }
  return { deleted, added };
}

export function changedLineCounts(
  ranges: readonly CodePeekProps[],
  evidence: SourceEvidenceResolver,
): { additions: number; deletions: number } {
  const added = new Set<string>();
  const deleted = new Set<string>();
  for (const range of ranges) {
    collectLines(added, evidence.changedLines(range.file, "head")?.added, range);
    collectLines(deleted, evidence.changedLines(range.file, "base")?.deleted, range);
  }
  return { additions: added.size, deletions: deleted.size };
}

function collectLines(
  result: Set<string>,
  lines: ReadonlySet<number> | undefined,
  range: Pick<CodePeekProps, "file" | "fromLine" | "toLine">,
) {
  if (!lines) return;
  for (const line of lines)
    if (line >= range.fromLine && line <= range.toLine) result.add(`${range.file}:${line}`);
}
