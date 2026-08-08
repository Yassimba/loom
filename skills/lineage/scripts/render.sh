#!/usr/bin/env bash
# render.sh — build the lineage viewer (plain lineage or before/after diff)
# from Mermaid sources and validate them.
#
# Usage: render.sh [-t "Change name"] [-o out.html] [-d details.json] [-r "base..tip"] [--open] <diagram.mmd> [more.mmd ...]
#
#   -t      Page title (default: Lineage). HTML-escaped for you.
#   -o      Output HTML path (default: first .mmd's name with .html).
#   -d      Details JSON (one entry per node id) feeding the click-to-inspect
#           drawer. Escaped for the script tag automatically.
#   -r      Diff range the page describes (e.g. "744774b..HEAD"), stamped into
#           the header. Default: the current commit of the repo containing the
#           first .mmd, with a "working tree" marker when dirty.
#   --open  Open the built page in the default browser.
#
# Each .mmd file becomes one tab (the tab bar hides itself for a single
# file); the filename (minus extension, capitalized) is the tab label.
# Sources are HTML-escaped automatically — write plain Mermaid.
#
# Validation: if a Chrome/Chromium/Edge binary is found, the built page is
# loaded headlessly and every diagram must render. Parse errors are printed
# and the script exits 1. Without a browser (or offline) validation is
# skipped with a warning.
set -euo pipefail

title="Lineage"
out=""
details=""
range=""
open_after=0
files=()
while [ $# -gt 0 ]; do
  case "$1" in
    -t) title="$2"; shift 2 ;;
    -o) out="$2"; shift 2 ;;
    -d) details="$2"; shift 2 ;;
    -r) range="$2"; shift 2 ;;
    --open) open_after=1; shift ;;
    -h|--help) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) files+=("$1"); shift ;;
  esac
done
if [ ${#files[@]} -eq 0 ]; then
  echo "usage: render.sh [-t title] [-o out.html] [-d details.json] [--open] <diagram.mmd>..." >&2
  exit 2
fi
for f in "${files[@]}"; do
  [ -f "$f" ] || { echo "error: no such file: $f" >&2; exit 2; }
done
if [ -n "$details" ] && [ ! -f "$details" ]; then
  echo "error: no such file: $details" >&2; exit 2
fi
[ -n "$out" ] || out="${files[0]%.*}.html"

template="$(cd "$(dirname "$0")/.." && pwd)/assets/viewer.html"
[ -f "$template" ] || { echo "error: template not found: $template" >&2; exit 2; }

html_escape() { sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/"/\&quot;/g'; }

sections_tmp="$(mktemp)"
trap 'rm -f "$sections_tmp"' EXIT
for f in "${files[@]}"; do
  label="$(basename "$f")"
  label="${label%.*}"
  label="$(printf '%s' "$label" | awk '{ print toupper(substr($0,1,1)) substr($0,2) }' | html_escape)"
  {
    printf '<section title="%s"><pre class="mermaid">\n' "$label"
    html_escape < "$f"
    printf '\n</pre></section>\n'
  } >> "$sections_tmp"
done

# JSON is embedded in a script tag: escaping < as \\u003c keeps it valid JSON
# and inert HTML (no </script> or comment openers can appear).
details_tmp="$(mktemp)"
trap 'rm -f "$sections_tmp" "$details_tmp"' EXIT
if [ -n "$details" ]; then
  sed -e 's/</\\u003c/g' "$details" > "$details_tmp"
else
  printf '{}' > "$details_tmp"
fi

# Provenance: what the page describes and when it was built.
prov="generated $(date +%F)"
srcdir="$(cd "$(dirname "${files[0]}")" && pwd)"
if [ -n "$range" ]; then
  prov="$prov · $range"
elif sha="$(git -C "$srcdir" rev-parse --short HEAD 2>/dev/null)"; then
  prov="$prov · @$sha"
  git -C "$srcdir" diff --quiet 2>/dev/null && git -C "$srcdir" diff --cached --quiet 2>/dev/null \
    || prov="$prov · working tree"
fi
prov_esc="$(printf '%s' "$prov" | html_escape)"

title_esc="$(printf '%s' "$title" | html_escape)"
awk -v secfile="$sections_tmp" -v detfile="$details_tmp" -v title="$title_esc" -v prov="$prov_esc" '
  /BLUEPRINT:DIAGRAMS:START/ { skip = 1; while ((getline line < secfile) > 0) print line; next }
  /BLUEPRINT:DIAGRAMS:END/   { skip = 0; next }
  skip { next }
  /BLUEPRINT:DETAILS/ {
    printf "<script type=\"application/json\" id=\"node-details\">\n"
    while ((getline line < detfile) > 0) print line
    printf "</script>\n"
    next
  }
  /<h1 id="title">/ { printf "  <h1 id=\"title\">%s</h1>\n", title; next }
  /<span class="prov" id="prov">/ { printf "  <span class=\"prov\" id=\"prov\">%s</span>\n", prov; next }
  { print }
' "$template" > "$out"
abs_out="$(cd "$(dirname "$out")" && pwd)/$(basename "$out")"
echo "built: $abs_out (${#files[@]} diagram(s))"

find_browser() {
  local c
  for c in \
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
    "/Applications/Chromium.app/Contents/MacOS/Chromium" \
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge" \
    google-chrome chromium chromium-browser microsoft-edge; do
    if command -v "$c" >/dev/null 2>&1; then printf '%s' "$c"; return 0; fi
  done
  return 1
}

if browser="$(find_browser)"; then
  dom="$("$browser" --headless=new --disable-gpu --virtual-time-budget=15000 \
        --dump-dom "file://$abs_out" 2>/dev/null || true)"
  n_errors="$(printf '%s\n' "$dom" | grep -c 'class="blueprint-error"' || true)"
  n_rendered="$(printf '%s\n' "$dom" | grep -c 'aria-roledescription=' || true)"
  if [ "$n_errors" -gt 0 ]; then
    echo "FAIL: $n_errors diagram(s) did not parse:" >&2
    printf '%s\n' "$dom" | awk '/class="blueprint-error"/ { f = 1 } f { print "  " $0 } f && /<\/pre>/ { f = 0 }' >&2
    exit 1
  elif [ "$n_rendered" -lt ${#files[@]} ]; then
    echo "warning: only $n_rendered/${#files[@]} diagrams rendered — offline or CDN blocked? Diagrams NOT validated." >&2
  else
    echo "OK: all ${#files[@]} diagram(s) parsed and rendered"
  fi
else
  echo "warning: no Chrome/Chromium/Edge found — skipped validation" >&2
fi

if [ "$open_after" -eq 1 ]; then
  if command -v open >/dev/null 2>&1; then open "$abs_out"
  elif command -v xdg-open >/dev/null 2>&1; then xdg-open "$abs_out"
  else echo "warning: no opener found; open $abs_out yourself" >&2
  fi
fi
