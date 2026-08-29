#!/usr/bin/env bash
# Run every mechanical check on a diagrams folder and rasterize each figure.
#
#   scripts/check-figures.sh ai-docs/explanations/<slug>/diagrams [png-out-dir]
#
# Per figure: self_check.py (diagram-design skin/a11y rules), verify-geometry.py
# (label masks clipped by later nodes), and a PNG at 1800px wide via
# rsvg-convert for the eyeball pass. Exits non-zero when any check fails.
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
dir="${1:?diagrams dir}"
png="${2:-$dir/png}"
mkdir -p "$png"
fail=0
for f in "$dir"/*.html; do
  [ -e "$f" ] || { echo "no .html in $dir"; exit 1; }
  b="$(basename "${f%.html}")"
  python3 "$here/self_check.py" "$f" | tail -1 || fail=1
  python3 "$here/verify-geometry.py" "$f" | grep -v '^Summary' && fail=1
  [ -f "$dir/$b.svg" ] || { echo "MISSING $dir/$b.svg"; fail=1; }
  if command -v rsvg-convert >/dev/null; then
    rsvg-convert -w 1800 "$dir/$b.svg" -o "$png/$b.png" || fail=1
  else
    echo "rsvg-convert not found; skipping PNG for $b (brew install librsvg)"
  fi
done
echo "PNGs in $png — view each one."
exit $fail
