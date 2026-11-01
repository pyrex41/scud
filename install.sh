#!/bin/sh
# Install the latest published SCUD binary. No coding harness is required.
# Run from a checkout: sh install.sh
set -eu

REPO="pyrex41/scud"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

case "$(uname -s)" in
    Linux) OS_NAME="linux" ;;
    Darwin) OS_NAME="darwin" ;;
    MINGW*|MSYS*|CYGWIN*) OS_NAME="windows" ;;
    *) echo "Unsupported operating system" >&2; exit 1 ;;
esac
case "$(uname -m)" in
    x86_64|amd64) ARCH_NAME="amd64" ;;
    aarch64|arm64) ARCH_NAME="arm64" ;;
    *) echo "Unsupported architecture" >&2; exit 1 ;;
esac

echo "Installing the latest SCUD task-workflow CLI..."
TAG=$(curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
if [ -z "$TAG" ]; then
    echo "Error: could not determine the latest published release" >&2
    exit 1
fi

ASSET="scud-${OS_NAME}-${ARCH_NAME}"
DEST="${INSTALL_DIR}/scud"
if [ "$OS_NAME" = "windows" ]; then
    ASSET="${ASSET}.exe"
    DEST="${DEST}.exe"
fi
mkdir -p "$INSTALL_DIR"
TEMP=$(mktemp "${INSTALL_DIR}/.scud-download.XXXXXX")
trap 'rm -f "$TEMP"' 0
trap 'exit 130' INT
trap 'exit 143' TERM HUP

URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"
echo "Downloading ${TAG} for ${OS_NAME}/${ARCH_NAME}..."
if ! curl -sSfL -o "$TEMP" "$URL"; then
    echo "Download failed; any existing SCUD binary was left unchanged." >&2
    echo "Source fallback (Go 1.27.3+): clone https://github.com/${REPO}, then run go install ./cmd/scud from that checkout." >&2
    exit 1
fi
chmod +x "$TEMP"
if [ "$OS_NAME" != "windows" ]; then
    if ! "$TEMP" --version >/dev/null; then
        echo "Downloaded binary could not run; existing installation was left unchanged." >&2
        exit 1
    fi
fi
mv -f "$TEMP" "$DEST"
echo "Installed ${TAG} at ${DEST}"

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) printf 'Add this directory to PATH: export PATH="%s:$PATH"\n' "$INSTALL_DIR" ;;
esac

cat <<'GUIDE'

Get started in your project:
  scud init
  scud status
  scud models list
  scud models set main 'a uniquely matching model name'
  scud models set fast 'another uniquely matching model name'
  scud parse product.md > tasks.json
  scud validate --input tasks.json
  scud import tasks.json

Model credentials are needed only for discovery/inference, not task CRUD.
sb is optional. Coding-agent execution is not part of SCUD v3.
Nonempty legacy tasks.scg files require explicit reviewed conversion;
installation does not modify project task files or model defaults.
Run scud --help and scud COMMAND --help for the current command surface.
GUIDE
