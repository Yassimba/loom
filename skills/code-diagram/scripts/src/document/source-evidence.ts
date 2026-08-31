import { execFileSync } from "node:child_process";
import { readFile, realpath } from "node:fs/promises";
import path from "node:path";

import type { CodePeekProps } from "../authoring/core";
import { patchChangedLines, type ChangedLines } from "./diff";
import type { SourceEvidenceResolver, SourceRange } from "./model";

export class EvidenceInputError extends Error {
  constructor(
    readonly code: string,
    readonly subject: string,
    detail: string,
  ) {
    super(`${subject}: ${detail}`);
  }
}

type DiffFile = { path: string; previousPath?: string; lines: ChangedLines | null };

export class SourceEvidenceService implements SourceEvidenceResolver {
  private readonly cache = new Map<string, Promise<SourceRange>>();

  private constructor(
    private readonly repo: string,
    private readonly revision: string | null,
    private readonly diffs: DiffFile[],
  ) {}

  static async create(repo: string): Promise<SourceEvidenceService> {
    let revision: string | null = null;
    try {
      revision = git(repo, ["rev-parse", "HEAD"]).trim();
    } catch {
      /* non-git repositories support head evidence only */
    }
    let diffs: DiffFile[] = [];
    if (revision) {
      try {
        diffs = parseDiff(
          git(repo, ["diff", "--no-ext-diff", "--find-renames", "--unified=0", "HEAD", "--"]),
        );
      } catch {
        diffs = [];
      }
    }
    return new SourceEvidenceService(repo, revision, diffs);
  }

  resolveRange(input: CodePeekProps): Promise<SourceRange> {
    const key = `${input.graph ?? "head"}:${input.file}:${input.fromLine}:${input.toLine}`;
    const pending = this.cache.get(key) ?? this.readRange(input);
    this.cache.set(key, pending);
    return pending;
  }

  changedLines(file: string, side: "base" | "head"): ChangedLines | null {
    const match = this.diffs.find((entry) =>
      side === "base" ? (entry.previousPath ?? entry.path) === file : entry.path === file,
    );
    return match?.lines ?? null;
  }

  private async readRange(input: CodePeekProps): Promise<SourceRange> {
    validateRelativePath(input.file);
    let text: string;
    if (input.graph === "base") {
      if (!this.revision)
        throw new EvidenceInputError(
          "EVIDENCE_BASE_UNAVAILABLE",
          input.file,
          "repository has no pinned HEAD",
        );
      try {
        const entry = git(this.repo, ["ls-tree", this.revision, "--", input.file]).trim();
        if (!entry) throw new Error("missing");
        const mode = entry.split(/\s+/)[0];
        if (mode !== "100644" && mode !== "100755")
          throw new EvidenceInputError(
            "EVIDENCE_FILE_INVALID",
            input.file,
            `unsupported git mode ${mode}`,
          );
        text = git(this.repo, ["show", `${this.revision}:${input.file}`]);
      } catch (error) {
        if (error instanceof EvidenceInputError) throw error;
        throw new EvidenceInputError(
          "EVIDENCE_FILE_MISSING",
          input.file,
          "file does not exist in pinned HEAD",
        );
      }
    } else {
      const repoPrefix = `${this.repo}${path.sep}`;
      const candidate = path.resolve(this.repo, input.file);
      if (candidate !== this.repo && !candidate.startsWith(repoPrefix))
        throw new EvidenceInputError(
          "EVIDENCE_FILE_INVALID",
          input.file,
          "resolves outside repository",
        );
      let resolved: string;
      try {
        resolved = await realpath(candidate);
      } catch {
        throw new EvidenceInputError("EVIDENCE_FILE_MISSING", input.file, "file does not exist");
      }
      if (resolved !== this.repo && !resolved.startsWith(repoPrefix))
        throw new EvidenceInputError(
          "EVIDENCE_FILE_INVALID",
          input.file,
          "symlink resolves outside repository",
        );
      text = await readFile(resolved, "utf8");
    }
    const lines = text.split(/\r\n|\n|\r/);
    if (input.toLine > lines.length)
      throw new EvidenceInputError(
        "EVIDENCE_RANGE_INVALID",
        input.file,
        `range ${input.fromLine}-${input.toLine} exceeds ${lines.length} lines`,
      );
    return {
      file: input.file,
      fromLine: input.fromLine,
      toLine: input.toLine,
      lines: lines
        .slice(input.fromLine - 1, input.toLine)
        .map((line, index) => ({ number: input.fromLine + index, text: line })),
    };
  }
}

function validateRelativePath(file: string) {
  if (!file || path.isAbsolute(file) || file.split(/[\\/]/).includes(".."))
    throw new EvidenceInputError(
      "EVIDENCE_FILE_INVALID",
      file,
      "must be a repository-relative path without traversal",
    );
}

function git(repo: string, args: string[]): string {
  return execFileSync("git", ["-C", repo, ...args], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
    maxBuffer: 20 * 1024 * 1024,
  });
}

function parseDiff(diff: string): DiffFile[] {
  const chunks = diff.split(/^diff --git /m).slice(1);
  return chunks
    .map((chunk) => {
      const current = /^a\/(.*?) b\/(.*?)$/m.exec(chunk);
      const renameFrom = /^rename from (.+)$/m.exec(chunk)?.[1];
      const renameTo = /^rename to (.+)$/m.exec(chunk)?.[1];
      const pathName = renameTo ?? current?.[2] ?? "";
      const previousPath = renameFrom ?? current?.[1];
      const binary = /^Binary files |^GIT binary patch/m.test(chunk);
      const hasHunk = /^@@ /m.test(chunk);
      return {
        path: pathName,
        ...(previousPath && previousPath !== pathName ? { previousPath } : {}),
        lines: binary || !hasHunk ? null : patchChangedLines(chunk),
      };
    })
    .filter((entry) => entry.path);
}
