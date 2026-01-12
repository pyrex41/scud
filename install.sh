#!/bin/bash
# SCUD CLI Installer
# Installs SCUD from source (no npm required)

set -e

echo "🚀 Installing SCUD CLI..."
echo

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust toolchain not found. Please install Rust first:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo "   source ~/.cargo/env"
    exit 1
fi

# Clone or update repository
if [ -d "scud" ]; then
    echo "📦 Updating SCUD repository..."
    cd scud
    git pull
else
    echo "📦 Cloning SCUD repository..."
    git clone https://github.com/pyrex41/scud.git
    cd scud
fi

# Build the CLI
echo "🔨 Building SCUD CLI..."
cargo build --release --quiet

# Install globally
echo "📦 Installing SCUD globally..."
./target/release/scud install

echo
echo "✅ SCUD CLI installed successfully!"
echo
echo "Next steps:"
echo "  cd your-project"
echo "  scud init"
echo
echo "Need help? Visit: https://github.com/pyrex41/scud"