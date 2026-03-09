#!/bin/sh
# SCUD + Rho Installer
# curl -sSf https://raw.githubusercontent.com/pyrex41/scud/master/install.sh | sh
set -e

SCUD_REPO="pyrex41/scud"
RHO_REPO="pyrex41/rho"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

detect_platform() {
    OS=$(uname -s)
    ARCH=$(uname -m)

    case "$OS" in
        Linux)  OS_NAME="linux" ;;
        Darwin) OS_NAME="macos" ;;
        MINGW*|MSYS*|CYGWIN*) OS_NAME="windows" ;;
        *) OS_NAME="" ;;
    esac

    case "$ARCH" in
        x86_64|amd64)  ARCH_NAME="x64" ;;
        aarch64|arm64) ARCH_NAME="arm64" ;;
        *) ARCH_NAME="" ;;
    esac
}

download_binary() {
    REPO="$1"
    BIN_NAME="$2"
    ASSET_PREFIX="$3"

    if [ "$OS_NAME" = "windows" ]; then
        ASSET="${ASSET_PREFIX}-${OS_NAME}-${ARCH_NAME}.exe"
        DEST="${INSTALL_DIR}/${BIN_NAME}.exe"
    else
        ASSET="${ASSET_PREFIX}-${OS_NAME}-${ARCH_NAME}"
        DEST="${INSTALL_DIR}/${BIN_NAME}"
    fi

    TAG=$(curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
    if [ -z "$TAG" ]; then
        return 1
    fi

    URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"
    echo "  Downloading ${BIN_NAME} ${TAG} (${OS_NAME}/${ARCH_NAME})..."

    if curl -sSfL -o "$DEST" "$URL"; then
        chmod +x "$DEST"
        echo "  Installed ${BIN_NAME} to ${DEST}"
        return 0
    else
        return 1
    fi
}

cargo_fallback() {
    BIN_NAME="$1"
    CRATE_NAME="$2"

    echo "  Prebuilt binary not available. Installing via cargo..."
    if ! command -v cargo > /dev/null 2>&1; then
        echo "  Rust not found. Installing via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        . "$HOME/.cargo/env"
    fi
    cargo install "$CRATE_NAME"
}

echo "Installing SCUD CLI + Rho harness..."
echo ""

detect_platform
mkdir -p "$INSTALL_DIR"

# Install scud
echo "scud:"
if [ -n "$OS_NAME" ] && [ -n "$ARCH_NAME" ]; then
    download_binary "$SCUD_REPO" "scud" "scud" || cargo_fallback "scud" "scud-cli"
else
    cargo_fallback "scud" "scud-cli"
fi

echo ""

# Install rho-cli
echo "rho-cli:"
if [ -n "$OS_NAME" ] && [ -n "$ARCH_NAME" ]; then
    download_binary "$RHO_REPO" "rho-cli" "rho-cli" || cargo_fallback "rho-cli" "rho-agent"
else
    cargo_fallback "rho-cli" "rho-agent"
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
