#!/usr/bin/env node

const { spawnSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const platform = process.platform;
const binaryName = platform === 'win32' ? 'scud.exe' : 'scud';
const binaryPath = path.join(__dirname, binaryName);

// Check if Rust binary exists
if (!fs.existsSync(binaryPath)) {
  console.error('Error: SCUD binary not found.');
  console.error('The installation may have failed. Try reinstalling:');
  console.error('  npm install -g scud-task');
  console.error('\nOr build manually:');
  console.error('  cd ' + path.dirname(__dirname));
  console.error('  cargo build --release');
  process.exit(1);
}

// Execute the Rust binary
const args = process.argv.slice(2);
const result = spawnSync(binaryPath, args, {
  stdio: 'inherit',
  shell: false
});

process.exit(result.status || 0);
