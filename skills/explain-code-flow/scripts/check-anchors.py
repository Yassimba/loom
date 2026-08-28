#!/usr/bin/env python3
"""Verify every `path:line` anchor in a walkthrough against the source tree.

    python3 check-anchors.py <repo-root> walkthrough.md [brief.md ...]

For each anchor `some/file.rs:123` (or a range `:123–130`) the script prints
the source line beside the claim, so drift is visible without opening files.
Exit 1 on a missing file, an ambiguous suffix, or an out-of-range line.
Pass `--quiet` to print only problems.

Anchors are matched by suffix, so `state.rs:17` resolves to the unique file
ending in `state.rs` under the repo root; ambiguous suffixes are reported.
"""
from __future__ import annotations

import re
import sys
from functools import lru_cache
from pathlib import Path

ANCHOR_RE = re.compile(r"`?(?P<path>[\w./-]+\.\w{1,5}):(?P<a>\d+)(?:[–-](?P<b>\d+))?`?")
SKIP_DIRS = {".git", "node_modules", "target", "ai-docs", ".venv", "dist", "build"}


@lru_cache(maxsize=None)
def index(root: Path) -> dict[str, list[Path]]:
    files: dict[str, list[Path]] = {}
    for p in root.rglob("*"):
        if p.is_file() and not any(part in SKIP_DIRS for part in p.parts):
            files.setdefault(p.name, []).append(p)
    return files


def resolve(root: Path, ref: str) -> Path | list[Path] | None:
    direct = root / ref
    if direct.is_file():
        return direct
    cands = [p for p in index(root).get(Path(ref).name, []) if str(p).endswith(ref)]
    if len(cands) == 1:
        return cands[0]
    return cands or None


def main() -> int:
    args = [a for a in sys.argv[1:] if a != "--quiet"]
    quiet = "--quiet" in sys.argv
    root = Path(args[0]).resolve()
    bad = 0
    for doc in args[1:]:
        for lineno, text in enumerate(Path(doc).read_text(encoding="utf-8").splitlines(), 1):
            for m in ANCHOR_RE.finditer(text):
                target = resolve(root, m.group("path"))
                tag = f"{doc}:{lineno} {m.group(0).strip('`')}"
                if target is None:
                    print(f"MISSING  {tag}: no file ends with {m.group('path')}"); bad += 1; continue
                if isinstance(target, list):
                    print(f"AMBIG    {tag}: {', '.join(str(p.relative_to(root)) for p in target)}"); bad += 1; continue
                lines = target.read_text(encoding="utf-8", errors="replace").splitlines()
                a, b = int(m.group("a")), int(m.group("b") or m.group("a"))
                if a < 1 or b > len(lines):
                    print(f"RANGE    {tag}: file has {len(lines)} lines"); bad += 1; continue
                if not quiet:
                    print(f"ok       {tag}: {lines[a-1].strip()[:90]}")
    print(f"Summary: {bad} problem(s).")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
