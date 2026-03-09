# SCUD + Rho Installer for Windows
# irm https://raw.githubusercontent.com/pyrex41/scud/master/install.ps1 | iex
$ErrorActionPreference = "Stop"

$ScudRepo = "pyrex41/scud"
$RhoRepo = "pyrex41/rho"
$InstallDir = if ($env:INSTALL_DIR) { $env:INSTALL_DIR } else { "$env:USERPROFILE\.local\bin" }

function Download-Binary {
    param($Repo, $BinName, $AssetPrefix)

    $Asset = "$AssetPrefix-windows-x64.exe"
    $Dest = "$InstallDir\$BinName.exe"

    try {
        $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
        $Tag = $Release.tag_name
    } catch {
        Write-Host "  Could not fetch latest release for $Repo"
        return $false
    }

    $Url = "https://github.com/$Repo/releases/download/$Tag/$Asset"
    Write-Host "  Downloading $BinName $Tag (windows/x64)..."

    try {
        Invoke-WebRequest -Uri $Url -OutFile $Dest -UseBasicParsing
        Write-Host "  Installed $BinName to $Dest"
        return $true
    } catch {
        Write-Host "  Download failed: $_"
        return $false
    }
}

function Cargo-Fallback {
    param($BinName, $CrateName)

    Write-Host "  Prebuilt binary not available. Installing via cargo..."
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Host "  Rust not found. Install from https://rustup.rs first."
        exit 1
    }
    cargo install $CrateName
}

Write-Host "Installing SCUD CLI + Rho harness..."
Write-Host ""

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

# Install scud
Write-Host "scud:"
if (-not (Download-Binary -Repo $ScudRepo -BinName "scud" -AssetPrefix "scud")) {
    Cargo-Fallback -BinName "scud" -CrateName "scud-cli"
}

Write-Host ""

# Install rho-cli
Write-Host "rho-cli:"
if (-not (Download-Binary -Repo $RhoRepo -BinName "rho-cli" -AssetPrefix "rho-cli")) {
    Cargo-Fallback -BinName "rho-cli" -CrateName "rho-agent"
}

Write-Host ""

# Add to PATH (both permanently and for current session)
if ($env:PATH -notlike "*$InstallDir*") {
    # Add to current session immediately
    $env:PATH = "$InstallDir;$env:PATH"
    Write-Host "Added $InstallDir to current session PATH."

    # Add permanently to user PATH
    $UserPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
    if ($UserPath -notlike "*$InstallDir*") {
        if ($UserPath) {
            [Environment]::SetEnvironmentVariable('PATH', "$InstallDir;$UserPath", 'User')
        } else {
            [Environment]::SetEnvironmentVariable('PATH', "$InstallDir", 'User')
        }
        Write-Host "Added $InstallDir to user PATH permanently."
    }
    Write-Host ""
}

Write-Host "Done! Run 'scud init' in any project to get started."
Write-Host "https://github.com/pyrex41/scud"
