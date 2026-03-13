#!/bin/sh
# SCUD Installer
# curl -sSf https://raw.githubusercontent.com/pyrex41/scud/master/install.sh | sh
set -e

REPO="pyrex41/scud"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

detect_platform() {
    OS=$(uname -s)
    ARCH=$(uname -m)

    case "$OS" in
        Linux)  OS_NAME="linux" ;;
        Darwin) OS_NAME="darwin" ;;
        MINGW*|MSYS*|CYGWIN*) OS_NAME="windows" ;;
        *) echo "Unsupported OS: $OS"; exit 1 ;;
    esac

    case "$ARCH" in
        x86_64|amd64)  ARCH_NAME="amd64" ;;
        aarch64|arm64) ARCH_NAME="arm64" ;;
        *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
    esac
}

echo "Installing SCUD CLI..."
echo ""

detect_platform
mkdir -p "$INSTALL_DIR"

TAG=$(curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
if [ -z "$TAG" ]; then
    echo "Error: could not determine latest release"
    exit 1
fi

if [ "$OS_NAME" = "windows" ]; then
    ASSET="scud-${OS_NAME}-${ARCH_NAME}.exe"
    DEST="${INSTALL_DIR}/scud.exe"
else
    ASSET="scud-${OS_NAME}-${ARCH_NAME}"
    DEST="${INSTALL_DIR}/scud"
fi

URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"
echo "Downloading scud ${TAG} (${OS_NAME}/${ARCH_NAME})..."

if curl -sSfL -o "$DEST" "$URL"; then
    chmod +x "$DEST"
    echo "Installed scud to ${DEST}"
else
    echo "Error: download failed"
    echo "  Go fallback: go install github.com/reuben/scud/cmd/scud@latest"
    exit 1
fi

echo ""

# Check PATH
case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        echo "Add ${INSTALL_DIR} to your PATH:"
        echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
        echo ""
        ;;
esac

echo "Done! Run 'scud init' in any project to get started."
echo "https://github.com/pyrex41/scud"
