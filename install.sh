#!/usr/bin/env sh
set -eu

# Loom bootstrap: ensure mise, sync the published tool manifest into
# mise's conf.d, install its exact pins (including the Loom CLI itself),
# then hand off to the guided setup. Tools update only when a new manifest
# lands on main and `loom update` re-syncs it.

NAME="loom"
REPO="Yassimba/loom"
MANIFEST_URL="https://raw.githubusercontent.com/${REPO}/main/manifest/loom.toml"
CONF_D="${HOME}/.config/mise/conf.d"

case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*)
    echo "$NAME: this looks like Git Bash/MSYS on Windows; use the PowerShell installer instead:" >&2
    echo '  powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/Yassimba/loom/main/install.ps1 | iex"' >&2
    exit 1 ;;
esac

command -v curl >/dev/null 2>&1 || { echo "$NAME: curl is required" >&2; exit 1; }

# 1. mise — the only thing this script installs itself.
if ! command -v mise >/dev/null 2>&1; then
  echo "$NAME: installing mise (https://mise.jdx.dev)..."
  curl -fsSL --retry 5 --retry-delay 3 https://mise.run | sh
  export PATH="${HOME}/.local/bin:${PATH}"
fi
command -v mise >/dev/null 2>&1 || { echo "$NAME: mise install failed" >&2; exit 1; }

# 2. Refresh the required core block — node and the Loom CLI — while keeping
#    any optional tools already chosen through the wizard. This also repairs
#    selections left incomplete by an interrupted or older bootstrap.
mkdir -p "$CONF_D"
selection="${CONF_D}/loom.toml"
tmp_manifest="$(mktemp)"
tmp_core="$(mktemp)"
tmp_selection="$(mktemp)"
trap 'rm -f "$tmp_manifest" "$tmp_core" "$tmp_selection"' EXIT INT TERM
curl -fsSL --retry 5 --retry-delay 3 "$MANIFEST_URL" -o "$tmp_manifest"
sed -n '/^# core:begin/,/^# core:end/p' "$tmp_manifest" > "$tmp_core"
if ! grep -q '^# core:begin' "$tmp_core" || ! grep -q '^# core:end' "$tmp_core"; then
  echo "$NAME: manifest is missing its core block" >&2
  exit 1
fi

if [ -f "$selection" ]; then
  if ! awk -v core="$tmp_core" '
    function emit_core(    line) {
      while ((getline line < core) > 0) print line
      close(core)
    }
    /^# core:begin/ { skipping = 1; next }
    skipping { if (/^# core:end/) skipping = 0; next }
    $0 == "[tools]" && !inserted { print; emit_core(); inserted = 1; next }
    { print }
    END { if (!inserted || skipping) exit 42 }
  ' "$selection" > "$tmp_selection"; then
    echo "$NAME: existing selection could not be safely refreshed" >&2
    exit 1
  fi
else
  {
    echo "# Managed by Loom: the selected tools from the published manifest."
    echo ""
    echo "[tools]"
    cat "$tmp_core"
  } > "$tmp_selection"
fi
mv "$tmp_selection" "$selection"
echo "$NAME: core tools synced to $selection"

# 3. Install the pins — node and the Loom CLI (plus any prior selection).
mise install --yes

# 4. Persist shell activation, so the tools are on PATH in new shells.
case "$(basename "${SHELL:-sh}")" in
  zsh)  profile="${HOME}/.zshrc"; activate='eval "$(mise activate zsh)"' ;;
  bash) profile="${HOME}/.bashrc"; activate='eval "$(mise activate bash)"' ;;
  fish) profile="${HOME}/.config/fish/config.fish"; activate='mise activate fish | source' ;;
  *)    profile=""; activate="" ;;
esac
if [ -n "$profile" ]; then
  mkdir -p "$(dirname "$profile")"
  touch "$profile"
  if ! grep -Fqx "$activate" "$profile"; then
    printf '\n%s\n' "$activate" >> "$profile"
    echo "$NAME: added mise activation to $profile"
  fi
else
  echo ""
  echo "$NAME: could not detect your shell; add mise activation to its profile" >&2
fi

# 5. Hand off to the guided setup with the freshly installed tools on PATH.
echo ""
# The README pipes this script into sh, so stdin is the download pipe rather
# than the user's terminal. Reconnect it before starting the interactive UI.
if [ ! -t 0 ] && [ -t 1 ] && ( : </dev/tty ) 2>/dev/null; then
  exec mise exec -- loom setup </dev/tty
fi
exec mise exec -- loom setup
