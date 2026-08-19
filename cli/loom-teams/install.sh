#!/usr/bin/env sh
set -eu

# loom-teams standalone installer (macOS/Linux).
#   curl -LsSf https://raw.githubusercontent.com/Yassimba/loom/main/cli/loom-teams/install.sh | sh
# Downloads the release binary pinned by the published manifest, verifies its
# checksum, and installs it to ~/.local/bin. Windows users:
#   powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/Yassimba/loom/main/cli/loom-teams/install.ps1 | iex"

NAME="loom-teams"
REPO="Yassimba/loom"
MANIFEST_URL="https://raw.githubusercontent.com/${REPO}/main/manifest/loom.toml"
BIN_DIR="${LOOM_TEAMS_INSTALL_DIR:-${HOME}/.local/bin}"

case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*)
    echo "$NAME: this looks like Git Bash/MSYS on Windows; use the PowerShell installer instead:" >&2
    echo '  powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/Yassimba/loom/main/cli/loom-teams/install.ps1 | iex"' >&2
    exit 1 ;;
esac

command -v curl >/dev/null 2>&1 || { echo "$NAME: curl is required" >&2; exit 1; }

os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Darwin)
    case "$arch" in
      arm64 | aarch64) target="aarch64-apple-darwin" ;;
      x86_64) target="x86_64-apple-darwin" ;;
      *) echo "$NAME: unsupported macOS architecture $arch" >&2; exit 1 ;;
    esac ;;
  Linux)
    case "$arch" in
      arm64 | aarch64) target="aarch64-unknown-linux-gnu" ;;
      x86_64) target="x86_64-unknown-linux-gnu" ;;
      *) echo "$NAME: unsupported Linux architecture $arch" >&2; exit 1 ;;
    esac ;;
  *) echo "$NAME: unsupported OS $os" >&2; exit 1 ;;
esac

# The published manifest pins the released tag; install exactly that.
tag="$(curl -fsSL --retry 5 --retry-delay 3 "$MANIFEST_URL" \
  | sed -n 's/^"github:Yassimba\/loom\[exe=loom-teams\]" = { version = "\([^"]*\)".*/\1/p')"
[ -n "$tag" ] || { echo "$NAME: could not read the release pin from the manifest" >&2; exit 1; }

asset="${NAME}-${target}.tar.gz"
url="https://github.com/${REPO}/releases/download/${tag}/${asset}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

echo "$NAME: downloading $tag for $target..."
curl -fsSL --retry 5 --retry-delay 3 "$url" -o "$tmp/$asset"
curl -fsSL --retry 5 --retry-delay 3 "$url.sha256" -o "$tmp/$asset.sha256"

expected="$(awk '{print $1}' "$tmp/$asset.sha256")"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$tmp/$asset" | awk '{print $1}')"
else
  actual="$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')"
fi
[ "$expected" = "$actual" ] || { echo "$NAME: checksum mismatch for $asset" >&2; exit 1; }

tar -xzf "$tmp/$asset" -C "$tmp"
mkdir -p "$BIN_DIR"
install -m 755 "$tmp/$NAME" "$BIN_DIR/$NAME"
echo "$NAME: installed $tag to $BIN_DIR/$NAME"

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *)
    echo ""
    echo "$NAME: $BIN_DIR is not on your PATH; add this to your shell profile:"
    echo "  export PATH=\"$BIN_DIR:\$PATH\"" ;;
esac

echo ""
echo "Next: run \`$NAME setup\` and sign in to Teams once."
