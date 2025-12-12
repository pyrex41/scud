#!/usr/bin/env node

/**
 * Post-install script for npm
 * Downloads pre-built Rust binary for the current platform
 */

const https = require('https');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const GITHUB_REPO = 'pyrex41/scud';
const BINARY_DIR = path.join(__dirname, '..', 'scud-cli', 'target', 'release');

function getPlatformInfo() {
  const platform = process.platform;
  const arch = process.arch;

  const platformMap = {
    'darwin-x64': 'scud-macos-x64',
    'darwin-arm64': 'scud-macos-arm64',
    'linux-x64': 'scud-linux-x64',
    'win32-x64': 'scud-windows-x64.exe',
  };

  const key = `${platform}-${arch}`;
  const assetName = platformMap[key];

  if (!assetName) {
    throw new Error(`Unsupported platform: ${platform}-${arch}`);
  }

  const binaryName = platform === 'win32' ? 'scud.exe' : 'scud';
  return { assetName, binaryName, platform, arch };
}

function getLatestReleaseUrl(assetName) {
  return new Promise((resolve, reject) => {
    const options = {
      hostname: 'api.github.com',
      path: `/repos/${GITHUB_REPO}/releases/latest`,
      headers: {
        'User-Agent': 'scud-task-installer'
      }
    };

    https.get(options, (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => {
        try {
          const release = JSON.parse(data);
          const asset = release.assets?.find(a => a.name === assetName);

          if (!asset) {
            reject(new Error(`Asset ${assetName} not found in latest release`));
            return;
          }

          resolve(asset.browser_download_url);
        } catch (error) {
          reject(error);
        }
      });
    }).on('error', reject);
  });
}

function downloadBinary(url, destPath) {
  return new Promise((resolve, reject) => {
    console.log(`📦 Downloading pre-built binary from ${url}...`);

    https.get(url, {
      headers: { 'User-Agent': 'scud-task-installer' },
      followRedirect: true
    }, (res) => {
      // Handle redirects
      if (res.statusCode === 302 || res.statusCode === 301) {
        return downloadBinary(res.headers.location, destPath).then(resolve).catch(reject);
      }

      if (res.statusCode !== 200) {
        reject(new Error(`Failed to download: HTTP ${res.statusCode}`));
        return;
      }

      const file = fs.createWriteStream(destPath);
      res.pipe(file);

      file.on('finish', () => {
        file.close();
        fs.chmodSync(destPath, 0o755); // Make executable
        resolve();
      });

      file.on('error', (err) => {
        fs.unlinkSync(destPath);
        reject(err);
      });
    }).on('error', reject);
  });
}

function buildFromSource() {
  console.log('🔨 Building from source with Cargo...');
  const scudCliDir = path.join(__dirname, '..', 'scud-cli');

  try {
    execSync('cargo build --release', {
      cwd: scudCliDir,
      stdio: 'inherit'
    });
    console.log('✅ Built successfully from source');
    return true;
  } catch (error) {
    console.error('❌ Failed to build from source');
    console.error('   Install Rust from: https://rustup.rs/');
    return false;
  }
}

async function main() {
  // Detect if running under Bun and postinstall was skipped
  const isBun = process.versions.bun !== undefined;

  try {
    const { assetName, binaryName, platform, arch } = getPlatformInfo();

    // Create target directory
    fs.mkdirSync(BINARY_DIR, { recursive: true });

    const binaryPath = path.join(BINARY_DIR, binaryName);

    // Try to download pre-built binary
    try {
      const downloadUrl = await getLatestReleaseUrl(assetName);
      await downloadBinary(downloadUrl, binaryPath);
      console.log(`✅ Pre-built binary installed for ${platform}-${arch}`);
    } catch (downloadError) {
      console.log(`⚠️  Could not download pre-built binary: ${downloadError.message}`);

      // Fallback to building from source
      if (!buildFromSource()) {
        console.log('\n⚠️  SCUD installed with limited functionality');
        console.log('   Some commands (tags, list, parse-prd, etc.) require the Rust CLI');

        if (isBun) {
          console.log('\n💡 Bun detected: Postinstall scripts are blocked by default.');
          console.log('   To fix this, run: npm install -g scud-task');
          console.log('   Or manually download the binary from:');
          console.log('   https://github.com/pyrex41/scud/releases/latest');
        } else {
          console.log('   Install Rust and run: cd scud-cli && cargo build --release');
        }
      }
    }

    // Show welcome message
    console.log(`
╭───────────────────────────────╮
│                               │
│  SCUD Task Manager installed! │
│                               │
╰───────────────────────────────╯

To get started in your project:

  1. Initialize SCUD:
     $ scud init

  2. Check status:
     $ scud status

  3. Start workflow (in Claude Code):
     $ /scud-pm

📚 Documentation:
   • README.md
   • https://github.com/${GITHUB_REPO}

💡 Commands:
   • scud init       - Initialize in project
   • scud tags       - List epics
   • scud status     - Check workflow state
   • scud help       - Show all commands

Happy building! 🎉
`);

  } catch (error) {
    console.error('❌ Installation error:', error.message);
    console.log('   Basic SCUD commands will still work');
    console.log('   For full functionality, install Rust: https://rustup.rs/');
  }
}

main();
