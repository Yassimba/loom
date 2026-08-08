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

# 2. The manifest's core block only — node and the Loom CLI. Everything
#    else is optional and chosen in the wizard, which appends to this file.
#    An existing Loom selection is left alone (`loom update` refreshes it).
mkdir -p "$CONF_D"
selection="${CONF_D}/loom.toml"
if [ ! -f "$selection" ]; then
  tmp_manifest="$(mktemp)"
  trap 'rm -f "$tmp_manifest"' EXIT INT TERM
  curl -fsSL --retry 5 --retry-delay 3 "$MANIFEST_URL" -o "$tmp_manifest"
  {
    echo "# Managed by Loom: the selected tools from the published manifest."
    echo ""
    echo "[tools]"
    sed -n '/^# core:begin/,/^# core:end/p' "$tmp_manifest"
  } > "$selection"
  echo "$NAME: core tools synced to $selection"
fi

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
exec mise exec -- loom setup
