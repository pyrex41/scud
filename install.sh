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

# Install rho-cli if not present
RHO_REPO="pyrex41/rho"
if ! command -v rho-cli >/dev/null 2>&1; then
    echo "Installing rho-cli (required for scud heavy)..."

    RHO_TAG=$(curl -sSf "https://api.github.com/repos/${RHO_REPO}/releases/latest" | grep '"tag_name"' | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
    if [ -n "$RHO_TAG" ]; then
        # rho uses x64/macos naming convention
        case "$ARCH_NAME" in
            amd64) RHO_ARCH="x64" ;;
            arm64) RHO_ARCH="arm64" ;;
        esac
        case "$OS_NAME" in
            darwin)  RHO_OS="macos" ;;
            linux)   RHO_OS="linux" ;;
            windows) RHO_OS="windows" ;;
        esac

        if [ "$OS_NAME" = "windows" ]; then
            RHO_ASSET="rho-cli-${RHO_OS}-${RHO_ARCH}.exe"
            RHO_DEST="${INSTALL_DIR}/rho-cli.exe"
        else
            RHO_ASSET="rho-cli-${RHO_OS}-${RHO_ARCH}"
            RHO_DEST="${INSTALL_DIR}/rho-cli"
        fi

        RHO_URL="https://github.com/${RHO_REPO}/releases/download/${RHO_TAG}/${RHO_ASSET}"
        if curl -sSfL -o "$RHO_DEST" "$RHO_URL"; then
            chmod +x "$RHO_DEST"
            echo "Installed rho-cli to ${RHO_DEST}"
        else
            echo "Warning: rho-cli download failed (scud heavy will not work)"
        fi
    else
        echo "Warning: could not determine latest rho-cli release"
    fi
    echo ""
fi

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
