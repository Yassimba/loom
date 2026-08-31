import { z } from "zod";
import {
  type CallStackEntry,
  callStackEntryAnchor,
  callStackEntrySchema,
} from "./authoring";
import {
  type DiffState,
  diffStateSchema,
  sourceRangeSchema,
} from "../../document/model";
import type { ChangedLines } from "../../document/diff";

export type CallStackSide = "base" | "head";

// Positional diff over two authored call stacks. Frames align by anchor
// identity, the way git aligns lines. The result is one unified stack:
// base-only frames are removed, head-only frames are added, shared frames
// are context. Order follows the head stack, with removed frames emitted
// before the added frames of the same hunk, as in a unified diff.

export interface CallStackDiffRow {
  entry: CallStackEntry;
  change: DiffState;
  // Depth in the row's own side: its index in the base or head stack. The
  // renderer turns depth transitions into tree-util connectors.
  depth: number;
}

export const callStackBrowserSchema = z.strictObject({
  title: z.string().optional(),
  rows: z.array(
    z.strictObject({
      entry: callStackEntrySchema,
      change: diffStateSchema,
      depth: z.int().nonnegative(),
      source: sourceRangeSchema,
    }),
  ),
});

export function diffCallStacks(
  base: readonly CallStackEntry[],
  head: readonly CallStackEntry[],
): CallStackDiffRow[] {
  const baseIds = base.map((entry) => callStackEntryAnchor(entry).id);
  const headIds = head.map((entry) => callStackEntryAnchor(entry).id);

  // Longest-common-subsequence table; lists are author-curated and short.
  const lcs: number[][] = Array.from({ length: base.length + 1 }, () =>
    new Array<number>(head.length + 1).fill(0),
  );
  for (let i = base.length - 1; i >= 0; i -= 1) {
    for (let j = head.length - 1; j >= 0; j -= 1) {
      lcs[i]![j] =
        baseIds[i] === headIds[j]
          ? lcs[i + 1]![j + 1]! + 1
          : Math.max(lcs[i + 1]![j]!, lcs[i]![j + 1]!);
    }
  }

  const rows: CallStackDiffRow[] = [];
  let i = 0;
  let j = 0;
  while (i < base.length || j < head.length) {
    if (i < base.length && j < head.length && baseIds[i] === headIds[j]) {
      // Shared frame: render the head entry, so the click opens new code.
      rows.push({ entry: head[j]!, change: "unchanged", depth: j });
      i += 1;
      j += 1;
    } else if (
      i < base.length &&
      (j >= head.length || lcs[i + 1]![j]! >= lcs[i]![j + 1]!)
    ) {
      rows.push({ entry: base[i]!, change: "removed", depth: i });
      i += 1;
    } else {
      rows.push({ entry: head[j]!, change: "added", depth: j });
      j += 1;
    }
  }
  return rows;
}

// Tree-util connectors ("│  ", "├─ ", "└─ ") derived from the row depths,
// exactly like the `tree` command draws them. The glyph column sits at
// depth-1; a continuation bar fills each shallower column whose branch
// continues below the row.
export function callStackConnectorPrefix(
  rows: readonly CallStackDiffRow[],
  index: number,
): string {
  const depth = rows[index]!.depth;
  if (depth === 0) return "";
  const continuesAt = (column: number): boolean => {
    for (let next = index + 1; next < rows.length; next += 1) {
      const nextDepth = rows[next]!.depth;
      if (nextDepth <= column) return false;
      if (nextDepth === column + 1) return true;
    }
    return false;
  };
  let prefix = "";
  for (let column = 0; column < depth - 1; column += 1) {
    prefix += continuesAt(column) ? "│  " : "   ";
  }
  return `${prefix}${continuesAt(depth - 1) ? "├─ " : "└─ "}`;
}

// Evidence rule: a "-" row is a claim of removal and must anchor a range
// that the change actually deletes lines from; a "+" row must anchor a
// range with added lines. This is the check that makes the markers honest —
// a frame listed on one side for contrast, over unchanged code, fails
// publish. Context rows carry no claim and stay free.
export function callStackEvidenceErrors(
  rows: readonly CallStackDiffRow[],
  changedLines: (
    file: string,
    side: CallStackSide,
  ) => ChangedLines | null,
): string[] {
  const errors: string[] = [];
  for (const row of rows) {
    if (row.change === "unchanged") continue;
    const anchor = callStackEntryAnchor(row.entry);
    const { file, fromLine, toLine } = anchor.peek.props;
    const side: CallStackSide = row.change === "removed" ? "base" : "head";
    const lines = changedLines(file, side);
    const relevant = row.change === "removed" ? lines?.deleted : lines?.added;
    let intersects = false;
    if (relevant) {
      for (let line = fromLine; line <= toLine; line += 1) {
        if (relevant.has(line)) {
          intersects = true;
          break;
        }
      }
    }
    if (intersects) continue;
    const marker = row.change === "removed" ? '"-"' : '"+"';
    const kind = row.change === "removed" ? "deleted" : "added";
    errors.push(
      `Frame "${anchor.id}" renders ${marker} but ${file}:${fromLine}-${toLine} ` +
        `contains no ${kind} lines in this change. Anchor the ${kind} call ` +
        `site, or list the frame on both sides.`,
    );
  }
  return errors;
}
