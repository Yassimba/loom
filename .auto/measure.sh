#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
python3 -m py_compile skills/explain-code-flow/scripts/*.py
python3 .auto/score.py
out="$(mktemp -d "${TMPDIR:-/tmp}/explain-flow-check.XXXXXX")"
start="$(python3 -c 'import time; print(time.perf_counter_ns())')"
python3 skills/explain-code-flow/scripts/example-figure.py "$out/example" >/dev/null
skills/explain-code-flow/scripts/check-figures.sh "$out" >/dev/null
end="$(python3 -c 'import time; print(time.perf_counter_ns())')"
rm -rf "$out"
python3 - "$start" "$end" <<'PY'
import sys
print(f"METRIC check_ms={(int(sys.argv[2])-int(sys.argv[1]))/1_000_000:.3f}")
print("METRIC quality_checks=1")
PY
