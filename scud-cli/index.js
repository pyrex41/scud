#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');

const platform = process.platform;
const binaryName = platform === 'win32' ? 'scud.exe' : 'scud';
const binaryPath = path.join(__dirname, 'bin', binaryName);

const args = process.argv.slice(2);

const child = spawn(binaryPath, args, {
  stdio: 'inherit'
});

child.on('exit', (code) => {
  process.exit(code);
});
