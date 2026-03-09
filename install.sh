#!/bin/sh
# SCUD + Rho Installer
# curl -sSf https://raw.githubusercontent.com/pyrex41/scud/master/install.sh | sh
set -e

echo "Installing SCUD CLI + Rho harness..."
echo ""

# Check for cargo, install Rust if missing
if ! command -v cargo > /dev/null 2>&1; then
    echo "Rust not found. Installing via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    . "$HOME/.cargo/env"
fi

echo "Installing scud-cli..."
cargo install scud-cli

echo ""
echo "Installing rho-cli (default AI harness)..."
if cargo install rho-agent; then
    echo "rho-cli installed."
else
    echo "Warning: rho-cli install failed. Install later with: cargo install rho-agent"
fi

echo ""
echo "Done! Run 'scud init' in any project to get started."
echo "https://github.com/pyrex41/scud"
