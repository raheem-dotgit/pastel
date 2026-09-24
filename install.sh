#!/bin/sh
# Install or update Pastel.
#   curl -fsSL https://raw.githubusercontent.com/raheem-dotgit/pastel/main/install.sh | sh
set -eu

REPO="raheem-dotgit/pastel"
BIN_DIR="$HOME/.local/bin"
BIN="$BIN_DIR/pastel"

case "$(uname -m)" in
    x86_64 | amd64) ARCH=x86_64 ;;
    aarch64 | arm64) ARCH=aarch64 ;;
    *) echo "Unsupported CPU architecture: $(uname -m)" >&2; exit 1 ;;
esac

URL="https://github.com/$REPO/releases/latest/download/pastel-$ARCH"
echo "Downloading Pastel ($ARCH)..."
mkdir -p "$BIN_DIR"
TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT
curl -fL --progress-bar "$URL" -o "$TMP"
chmod +x "$TMP"

# Stop a running copy so the new version takes over.
if [ -x "$BIN" ]; then
    "$BIN" quit 2>/dev/null || true
    sleep 1
fi
mv "$TMP" "$BIN"
trap - EXIT

"$BIN" install

missing=""
ldconfig -p 2>/dev/null | grep -q libadwaita-1.so.0 || missing="$missing libadwaita-1-0"
ldconfig -p 2>/dev/null | grep -q libgtk-4.so.1 || missing="$missing libgtk-4-1"
command -v wl-copy >/dev/null 2>&1 || missing="$missing wl-clipboard"
if [ -n "$missing" ]; then
    echo
    echo "Pastel needs these packages. Install them with:"
    echo "  sudo apt install$missing"
    exit 0
fi

"$BIN" show
echo
echo "Pastel is installed and running. Press Super+V to open it."
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "Note: $BIN_DIR is not in your PATH; run Pastel as $BIN" ;;
esac
