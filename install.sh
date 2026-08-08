#!/usr/bin/env sh
set -eu

# ai-setup bootstrap: ensure mise, sync the published tool manifest into
# mise's conf.d, install its exact pins (including the ai-setup CLI itself),
# then hand off to the guided setup. Tools update only when a new manifest
# lands on main and `ai-setup update` re-syncs it.

NAME="ai-setup"
REPO="Yassimba/ai-setup"
MANIFEST_URL="https://raw.githubusercontent.com/${REPO}/main/manifest/ai-setup.toml"
CONF_D="${HOME}/.config/mise/conf.d"

case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*)
    echo "$NAME: this looks like Git Bash/MSYS on Windows; use the PowerShell installer instead:" >&2
    echo '  powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/Yassimba/ai-setup/main/install.ps1 | iex"' >&2
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

# 2. The published manifest, merged by mise without touching the user's own
#    config.toml (that file stays theirs, as a personal overlay).
mkdir -p "$CONF_D"
curl -fsSL --retry 5 --retry-delay 3 "$MANIFEST_URL" -o "${CONF_D}/ai-setup.toml"
echo "$NAME: manifest synced to ${CONF_D}/ai-setup.toml"

# 3. Install the pins — node, pi, the ai-setup CLI, and the rest.
mise install --yes

# 4. Shell activation, so the tools are on PATH in new shells.
case "$(basename "${SHELL:-sh}")" in
  zsh)  profile="~/.zshrc";  activate='eval "$(mise activate zsh)"' ;;
  bash) profile="~/.bashrc"; activate='eval "$(mise activate bash)"' ;;
  fish) profile="~/.config/fish/config.fish"; activate='mise activate fish | source' ;;
  *)    profile="your shell profile"; activate='eval "$(mise activate <shell>)"' ;;
esac
if ! mise doctor 2>/dev/null | grep -q "activated: yes"; then
  echo ""
  echo "$NAME: add mise activation to ${profile} if it is not there yet:"
  echo "  ${activate}"
fi

# 5. Hand off to the guided setup with the freshly installed tools on PATH.
echo ""
exec mise exec -- ai-setup setup
