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
mise_bin="$(command -v mise)"
mise_dir="$(dirname "$mise_bin")"

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
mise -C "$HOME" install --yes

# 4. Persist shell activation, so the tools are on PATH in new shells.
case "$(basename "${SHELL:-sh}")" in
  zsh)  profile="${HOME}/.zshrc"; activate="$(printf 'export PATH="%s:$PATH"; eval "$("%s" activate zsh)"' "$mise_dir" "$mise_bin")" ;;
  bash) profile="${HOME}/.bashrc"; activate="$(printf 'export PATH="%s:$PATH"; eval "$("%s" activate bash)"' "$mise_dir" "$mise_bin")" ;;
  fish) profile="${HOME}/.config/fish/config.fish"; activate="$(printf 'fish_add_path "%s"; "%s" activate fish | source' "$mise_dir" "$mise_bin")" ;;
  *)    profile=""; activate="" ;;
esac
if [ -n "$profile" ]; then
  mkdir -p "$(dirname "$profile")"
  touch "$profile"
  if ! grep -Fqx "$activate" "$profile"; then
    printf '\n%s\n' "$activate" >> "$profile"
    echo "$NAME: added mise activation to $profile"
    activation_added=1
  fi
else
  echo ""
  echo "$NAME: could not detect your shell; add mise activation to its profile" >&2
fi

# After the guided setup: a shell opened before this run has no mise hook yet,
# so `loom` is "command not found" there until it restarts. Say so once.
finish() {
  if [ "${activation_added:-0}" = 1 ]; then
    echo ""
    echo "$NAME: open a new shell (or run: exec $(basename "${SHELL:-sh}")) so loom and the tools are on PATH"
  fi
  exit "$1"
}

# 5. Hand off to the guided setup with the freshly installed tools on PATH.
echo ""
# The README pipes this script into sh, so stdin is the download pipe rather
# than the user's terminal. Reconnect it to the terminal device itself: the
# pty behind stderr first (kqueue on macOS cannot poll the /dev/tty alias),
# then /dev/tty. A scripted install (flags, no terminal) hands off as is;
# a guided install with no terminal at all is told what to run instead.
if [ ! -t 0 ]; then
  terminal="$(tty 0<&2 2>/dev/null)" || terminal=""
  case "$terminal" in
    /dev/*) ;;
    *) if ( : </dev/tty ) 2>/dev/null; then terminal=/dev/tty; else terminal=""; fi ;;
  esac
  if [ -n "$terminal" ] && [ -t 1 ]; then
    mise -C "$HOME" exec -- loom setup "$@" <"$terminal"
    finish $?
  fi
  if [ "$#" -eq 0 ]; then
    echo "$NAME: installed, but there is no terminal here for the guided setup. Open a shell and run: loom" >&2
    exit 1
  fi
fi
mise -C "$HOME" exec -- loom setup "$@"
finish $?
