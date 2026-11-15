#!/usr/bin/env bash
# Probe one tach rule: plant one snippet in one file, run tach, then undo the change.
#
# Usage (run from the directory that holds tach.toml):
#   probe.sh [--external] <file> "<python snippet>"
#
# An existing file gets the snippet appended and is restored afterwards. A missing file is
# created with the snippet and deleted afterwards (use this to probe code outside any module).
# For a snippet of several lines, pass a $'...' string with \n.
#
# Exit 0: tach failed, so the rule is red as wanted.
# Exit 1: tach passed, so the rule did not fire; the config has a bug.
# Exit 2: wrong usage.
set -euo pipefail

command_name="check"
if [[ "${1:-}" == "--external" ]]; then
  command_name="check-external"
  shift
fi
if [[ $# -ne 2 ]]; then
  echo "usage: probe.sh [--external] <file> \"<python snippet>\"" >&2
  exit 2
fi

target="$1"
snippet="$2"
backup="$(mktemp)"
if [[ -f "$target" ]]; then
  cp "$target" "$backup"
  # The undo runs on every exit path, also on Ctrl-C, so the planted snippet never stays behind.
  trap 'cp "$backup" "$target"; rm -f "$backup"' EXIT
  printf '\n%s\n' "$snippet" >>"$target"
else
  trap 'rm -f "$target" "$backup"' EXIT
  printf '%s\n' "$snippet" >"$target"
fi

if command -v uv >/dev/null 2>&1 && [[ -f pyproject.toml || -f uv.lock ]]; then
  tach_cmd=(uv run tach)
else
  tach_cmd=(tach)
fi

echo "probe: tach $command_name with a planted snippet in $target"
if "${tach_cmd[@]}" "$command_name"; then
  echo "probe: GREEN - the rule did not fire" >&2
  exit 1
fi
echo "probe: RED - the rule fired"
