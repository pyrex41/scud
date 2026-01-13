use anyhow::Result;
use colored::Colorize;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn run() -> Result<()> {
    println!("{}", "Installing SCUD...".blue().bold());

    // Check if cargo is available
    println!("Checking for Rust toolchain...");
    let cargo_check = Command::new("cargo").arg("--version").output();
    match cargo_check {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            println!("  {} {}", "✓".green(), version.trim());
        }
        _ => {
            println!("  {} Rust toolchain not found", "✗".red());
            println!("  Please install Rust from https://rustup.rs/");
            anyhow::bail!("Rust toolchain required");
        }
    }

    // Build the release binary
    println!("Building SCUD binary...");
    let current_dir = env::current_dir()?;
    let build_dir = current_dir.parent().unwrap_or_else(|| Path::new("."));
    let build_result = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(build_dir)
        .status();

    match build_result {
        Ok(status) if status.success() => {
            println!("  {} Binary built successfully", "✓".green());
        }
        _ => {
            println!("  {} Failed to build binary", "✗".red());
            anyhow::bail!("Build failed");
        }
    }

    // Find and install the binary
    let current_exe = env::current_exe()?;
    let binary_name = if cfg!(windows) { "scud.exe" } else { "scud" };

    // The binary should be in target/release/ relative to the repo root
    let repo_root = current_exe
        .parent() // scud-cli/target/release/
        .and_then(|p| p.parent()) // scud-cli/target/
        .and_then(|p| p.parent()) // scud-cli/
        .and_then(|p| p.parent()) // repo root
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let source_path = repo_root
        .join("scud-cli")
        .join("target")
        .join("release")
        .join(binary_name);

    if !source_path.exists() {
        println!(
            "  {} Binary not found at: {}",
            "✗".red(),
            source_path.display()
        );
        anyhow::bail!("Binary not found after build");
    }

    // Install to ~/.cargo/bin/ (standard Rust bin location)
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let cargo_bin_dir = home_dir.join(".cargo").join("bin");
    fs::create_dir_all(&cargo_bin_dir)?;

    let dest_path = cargo_bin_dir.join(binary_name);

    // Remove existing binary if it exists
    if dest_path.exists() {
        fs::remove_file(&dest_path)?;
    }

    // Copy the binary
    fs::copy(&source_path, &dest_path)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dest_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest_path, perms)?;
    }

    println!(
        "  {} Binary installed to: {}",
        "✓".green(),
        dest_path.display()
    );

    // Verify installation
    let verify_result = Command::new(&dest_path).arg("--version").output();
    match verify_result {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            println!(
                "  {} Installation verified: {}",
                "✓".green(),
                version.trim()
            );
        }
        _ => {
            println!("  {} Installation verification failed", "✗".red());
        }
    }

    println!();
    println!("{}", "✅ SCUD installed successfully!".green().bold());
    println!();
    println!("Next steps:");
    println!("  1. Run: scud init");
    println!("  2. Set required environment variables");
    println!("  3. Start using SCUD!");

    Ok(())
}
