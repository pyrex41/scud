#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const platform = process.platform;
const arch = process.arch;

console.log(`Installing SCUD for ${platform}-${arch}...`);

// Check if cargo is available
try {
  execSync('cargo --version', { stdio: 'ignore' });
} catch (error) {
  console.error('Error: Cargo (Rust toolchain) is not installed.');
  console.error('Please install Rust from https://rustup.rs/');
  process.exit(1);
}

// Build the Rust binary
try {
  console.log('Building SCUD from source...');
  execSync('cargo build --release', {
    stdio: 'inherit',
    cwd: __dirname
  });

  // Copy binary to bin directory
  const binDir = path.join(__dirname, 'bin');
  const binaryName = platform === 'win32' ? 'scud.exe' : 'scud';
  const sourcePath = path.join(__dirname, 'target', 'release', binaryName);
  const destPath = path.join(binDir, binaryName);

  // Verify source binary exists
  if (!fs.existsSync(sourcePath)) {
    throw new Error(`Compiled binary not found at: ${sourcePath}`);
  }

  // Create bin directory if it doesn't exist
  if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir, { recursive: true });
  }

  // Remove any existing binary (in case of reinstall)
  if (fs.existsSync(destPath)) {
    fs.unlinkSync(destPath);
  }

  // Copy the compiled binary
  fs.copyFileSync(sourcePath, destPath);

  // Make executable on Unix-like systems
  if (platform !== 'win32') {
    fs.chmodSync(destPath, 0o755);
  }

  console.log(`✓ Binary installed at: ${destPath}`);

  console.log('✅ SCUD installed successfully!');
  console.log(`Run 'scud --help' to get started.`);
} catch (error) {
  console.error('Error building SCUD:', error.message);
  process.exit(1);
}
