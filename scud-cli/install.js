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

  // Create bin directory
  const binDir = path.join(__dirname, 'bin');
  if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir, { recursive: true });
  }

  // Copy binary to bin directory
  const binaryName = platform === 'win32' ? 'scud.exe' : 'scud';
  const sourcePath = path.join(__dirname, 'target', 'release', binaryName);
  const destPath = path.join(binDir, binaryName);

  fs.copyFileSync(sourcePath, destPath);

  // Make executable on Unix-like systems
  if (platform !== 'win32') {
    fs.chmodSync(destPath, 0o755);
  }

  console.log('✅ SCUD installed successfully!');
  console.log(`Run 'scud --help' to get started.`);
} catch (error) {
  console.error('Error building SCUD:', error.message);
  process.exit(1);
}
