#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
python3 -m py_compile skills/explain-code-flow/scripts/*.py
# Required workflow contracts must remain discoverable after prompt refactors.
grep -q 'production code' skills/explain-code-flow/SKILL.md
grep -q 'final result' skills/explain-code-flow/SKILL.md
grep -q 'Overview' skills/explain-code-flow/SKILL.md
grep -q 'Spine' skills/explain-code-flow/SKILL.md
grep -q 'parallel' skills/explain-code-flow/SKILL.md
grep -q 'check-anchors.py' skills/explain-code-flow/SKILL.md
grep -q 'check-figures.sh' skills/explain-code-flow/SKILL.md
grep -q 'value in' skills/explain-code-flow/SKILL.md
# A failed self-check must propagate through the figure wrapper.
tmp="$(mktemp -d "${TMPDIR:-/tmp}/explain-flow-quality.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
python3 skills/explain-code-flow/scripts/example-figure.py "$tmp/good" >/dev/null
skills/explain-code-flow/scripts/check-figures.sh "$tmp" >/dev/null
cp "$tmp/good.html" "$tmp/bad.html"
cp "$tmp/good.svg" "$tmp/bad.svg"
python3 - "$tmp/bad.html" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); p.write_text(p.read_text().replace('role="img"', ''), encoding='utf-8')
PY
if skills/explain-code-flow/scripts/check-figures.sh "$tmp" >/dev/null 2>&1; then
  echo 'check-figures.sh masked an invalid SVG' >&2
  exit 1
fi
