use anyhow::Result;
use colored::Colorize;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::commands::spawn::terminal::{find_harness_binary, Harness};
use crate::models::task::TaskStatus;
use crate::storage::Storage;

/// Diagnostic issue severity
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
    Critical,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Warning => "WARNING",
            Severity::Error => "ERROR",
            Severity::Critical => "CRITICAL",
        }
    }
}

/// A diagnostic issue found by the doctor command
#[derive(Debug, Clone)]
pub struct DiagnosticIssue {
    pub severity: Severity,
    pub epic_tag: String,
    pub task_id: Option<String>,
    pub message: String,
    pub suggestion: String,
}

/// Results from running diagnostics
#[derive(Debug, Default)]
pub struct DiagnosticResults {
    pub issues: Vec<DiagnosticIssue>,
    pub blocked_by_cancelled: Vec<(String, String, String)>, // (epic, task_id, blocked_dep)
    pub blocked_by_missing: Vec<(String, String, String)>,   // (epic, task_id, missing_dep)
    pub orphan_in_progress: Vec<(String, String)>, // (epic, task_id) - in-progress >threshold without activity
    pub missing_active_epic: bool,
    pub corrupt_files: Vec<String>,
}

impl DiagnosticResults {
    pub fn has_issues(&self) -> bool {
        !self.issues.is_empty()
            || !self.blocked_by_cancelled.is_empty()
            || !self.blocked_by_missing.is_empty()
            || !self.orphan_in_progress.is_empty()
            || self.missing_active_epic
            || !self.corrupt_files.is_empty()
    }

    pub fn critical_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Critical)
            .count()
            + self.corrupt_files.len()
    }

    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count()
            + self.blocked_by_cancelled.len()
            + self.blocked_by_missing.len()
    }

    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count()
            + self.orphan_in_progress.len()
            + if self.missing_active_epic { 1 } else { 0 }
    }
}

pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    stale_hours: f64,
    fix: bool,
) -> Result<()> {
    run_workflow_diagnostics(project_root, tag, stale_hours, fix)
}

fn run_workflow_diagnostics(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    stale_hours: f64,
    fix: bool,
) -> Result<()> {
    println!(
        "{}",
        "[EXPERIMENTAL] SCUD Doctor - Workflow Diagnostics"
            .blue()
            .bold()
    );
    println!("{}", "=".repeat(60).blue());
    println!();

    let storage = Storage::new(project_root);

    // Check if storage files exist and are readable
    let tasks_result = storage.load_tasks();

    let mut results = DiagnosticResults::default();

    // Check for corrupt/missing files
    if let Err(ref e) = tasks_result {
        results.corrupt_files.push(format!("tasks file: {}", e));
    }

    // Check active epic
    match storage.get_active_group() {
        Ok(Some(_)) => {}
        Ok(None) => {
            results.missing_active_epic = true;
        }
        Err(_) => {
            results.missing_active_epic = true;
        }
    }

    // Validate rho backend availability when using rho-first orchestration.
    if let Err(e) = find_harness_binary(Harness::Rho) {
        if fix {
            println!(
                "{} rho-cli not found, attempting install...",
                "⟳".blue()
            );
            let installed = try_install_rho_binary() || try_install_rho_cargo();
            if installed {
                println!(
                    "{} rho-cli installed successfully",
                    "✓".green()
                );
            } else {
                results.issues.push(DiagnosticIssue {
                    severity: Severity::Critical,
                    epic_tag: "runtime".to_string(),
                    task_id: None,
                    message: format!("Rho backend check failed: {}", e),
                    suggestion:
                        "Auto-install failed. Install manually: cargo install rho-agent"
                            .to_string(),
                });
            }
        } else {
            results.issues.push(DiagnosticIssue {
                severity: Severity::Critical,
                epic_tag: "runtime".to_string(),
                task_id: None,
                message: format!("Rho backend check failed: {}", e),
                suggestion:
                    "Run 'scud doctor workflow --fix' to auto-install, or: cargo install rho-agent"
                        .to_string(),
            });
        }
    }

    // If we couldn't load tasks, show what we found and exit
    if !results.corrupt_files.is_empty() {
        print_results(&results, fix);
        return Ok(());
    }

    let mut all_tasks = tasks_result?;

    // Filter to specific tag if provided
    let epic_tags: Vec<String> = if let Some(t) = tag {
        if all_tasks.contains_key(t) {
            vec![t.to_string()]
        } else {
            anyhow::bail!("Phase '{}' not found", t);
        }
    } else {
        all_tasks.keys().cloned().collect()
    };

    // Run diagnostics on each epic
    for epic_tag in &epic_tags {
        let epic = match all_tasks.get(epic_tag) {
            Some(e) => e,
            None => continue,
        };

        // Build set of all task IDs for dependency checking
        let all_task_ids: HashSet<_> = epic.tasks.iter().map(|t| t.id.clone()).collect();

        for task in &epic.tasks {
            // Check for orphan in-progress tasks (in-progress for too long without activity)
            if task.status == TaskStatus::InProgress {
                if let Some(ref updated_at) = task.updated_at {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(updated_at) {
                        let hours =
                            (chrono::Utc::now().signed_duration_since(dt)).num_hours() as f64;
                        if hours > stale_hours {
                            results
                                .orphan_in_progress
                                .push((epic_tag.clone(), task.id.clone()));
                        }
                    }
                }
            }

            // Check for dependencies on cancelled/blocked tasks
            if task.status == TaskStatus::Pending {
                for dep_id in &task.dependencies {
                    // Check if dependency exists
                    if !all_task_ids.contains(dep_id) {
                        results.blocked_by_missing.push((
                            epic_tag.clone(),
                            task.id.clone(),
                            dep_id.clone(),
                        ));
                        continue;
                    }

                    // Check if dependency is cancelled or blocked
                    if let Some(dep_task) = epic.get_task(dep_id) {
                        match dep_task.status {
                            TaskStatus::Cancelled => {
                                results.blocked_by_cancelled.push((
                                    epic_tag.clone(),
                                    task.id.clone(),
                                    dep_id.clone(),
                                ));
                            }
                            TaskStatus::Blocked => {
                                results.issues.push(DiagnosticIssue {
                                    severity: Severity::Warning,
                                    epic_tag: epic_tag.clone(),
                                    task_id: Some(task.id.clone()),
                                    message: format!(
                                        "Task {} depends on blocked task {}",
                                        task.id, dep_id
                                    ),
                                    suggestion: format!(
                                        "Resolve blocker for {} or remove dependency",
                                        dep_id
                                    ),
                                });
                            }
                            TaskStatus::Deferred => {
                                results.issues.push(DiagnosticIssue {
                                    severity: Severity::Warning,
                                    epic_tag: epic_tag.clone(),
                                    task_id: Some(task.id.clone()),
                                    message: format!(
                                        "Task {} depends on deferred task {}",
                                        task.id, dep_id
                                    ),
                                    suggestion: format!("Un-defer {} or update dependency", dep_id),
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    // Apply fixes if requested
    if fix && results.has_issues() {
        println!("{}", "Attempting auto-fixes...".yellow());
        println!();

        let mut fixed_count = 0;

        // Fix orphan in-progress tasks (reset to pending)
        for (epic_tag, task_id) in &results.orphan_in_progress {
            if let Some(epic) = all_tasks.get_mut(epic_tag) {
                if let Some(task) = epic.get_task_mut(task_id) {
                    task.set_status(TaskStatus::Pending);
                    println!(
                        "{} Reset stale in-progress task to pending: {}",
                        "✓".green(),
                        task_id.cyan()
                    );
                    fixed_count += 1;
                }
            }
        }

        if fixed_count > 0 {
            storage.save_tasks(&all_tasks)?;
            println!();
            println!("{} {} issue(s) fixed", "✓".green(), fixed_count);
        } else {
            println!(
                "{}",
                "No auto-fixable issues found. Manual intervention required.".yellow()
            );
        }
        println!();
    }

    print_results(&results, fix);

    Ok(())
}

fn print_results(results: &DiagnosticResults, fix_attempted: bool) {
    if !results.has_issues() {
        println!(
            "{}",
            "✓ No issues found! Workflow is healthy.".green().bold()
        );
        return;
    }

    // Print critical issues (corrupt files)
    if !results.corrupt_files.is_empty() {
        println!("{}", "CRITICAL: File Issues".red().bold());
        println!("{}", "-".repeat(40).red());
        for file_issue in &results.corrupt_files {
            println!("  {} {}", "✗".red(), file_issue);
        }
        println!();
        print_recovery_instructions();
        return;
    }

    // Print blocked by cancelled
    if !results.blocked_by_cancelled.is_empty() {
        println!("{}", "Tasks Blocked by Cancelled Dependencies".red().bold());
        println!("{}", "-".repeat(40).red());
        for (epic, task_id, dep_id) in &results.blocked_by_cancelled {
            println!(
                "  {} {} depends on cancelled task {}",
                "✗".red(),
                task_id.cyan(),
                dep_id.yellow()
            );
            println!(
                "    {}",
                format!(
                    "→ Remove dependency or un-cancel {} (in epic {})",
                    dep_id, epic
                )
                .dimmed()
            );
        }
        println!();
    }

    // Print blocked by missing
    if !results.blocked_by_missing.is_empty() {
        println!("{}", "Tasks with Missing Dependencies".red().bold());
        println!("{}", "-".repeat(40).red());
        for (epic, task_id, dep_id) in &results.blocked_by_missing {
            println!(
                "  {} {} depends on non-existent task {}",
                "✗".red(),
                task_id.cyan(),
                dep_id.yellow()
            );
            println!(
                "    {}",
                format!("→ Remove dependency from {} (in epic {})", task_id, epic).dimmed()
            );
        }
        println!();
    }

    // Print orphan in-progress
    if !results.orphan_in_progress.is_empty() {
        println!(
            "{}",
            "Stale In-Progress Tasks (no activity)".yellow().bold()
        );
        println!("{}", "-".repeat(40).yellow());
        for (epic, task_id) in &results.orphan_in_progress {
            println!(
                "  {} {} in {} - in-progress but no recent activity",
                "⚠".yellow(),
                task_id.cyan(),
                epic.dimmed()
            );
            if !fix_attempted {
                println!(
                    "    {}",
                    format!(
                        "→ scud set-status {} pending -t {}  # or done if complete",
                        task_id, epic
                    )
                    .dimmed()
                );
            }
        }
        println!();
    }

    // Print missing active epic
    if results.missing_active_epic {
        println!("{}", "No Active Phase Set".yellow().bold());
        println!("{}", "-".repeat(40).yellow());
        println!("  {} No active epic/tag is set", "⚠".yellow());
        println!(
            "    {}",
            "→ scud tags <epic-name>  # to set active epic".dimmed()
        );
        println!();
    }

    // Print other issues
    for issue in &results.issues {
        let (icon, color_fn): (&str, fn(&str) -> colored::ColoredString) = match issue.severity {
            Severity::Critical => ("✗", |s: &str| s.red()),
            Severity::Error => ("✗", |s: &str| s.red()),
            Severity::Warning => ("⚠", |s: &str| s.yellow()),
        };

        println!(
            "  {} [{}] {}",
            color_fn(icon),
            issue.severity.as_str(),
            issue.message
        );
        if let Some(ref task_id) = issue.task_id {
            println!(
                "    Task: {} in {}",
                task_id.cyan(),
                issue.epic_tag.dimmed()
            );
        }
        println!("    {}", format!("→ {}", issue.suggestion).dimmed());
    }

    // Summary
    println!();
    println!("{}", "Summary".blue().bold());
    println!("{}", "-".repeat(40).blue());
    println!(
        "  Critical: {}  Errors: {}  Warnings: {}",
        results.critical_count().to_string().red(),
        results.error_count().to_string().yellow(),
        results.warning_count().to_string().blue()
    );

    if !fix_attempted && !results.orphan_in_progress.is_empty() {
        println!();
        println!("{}", "To auto-fix recoverable issues, run:".blue());
        println!("  scud doctor --fix");
    }
}

fn print_recovery_instructions() {
    println!();
    println!("{}", "=".repeat(60).red());
    println!("{}", "RECOVERY INSTRUCTIONS".red().bold());
    println!("{}", "=".repeat(60).red());
    println!();
    println!("The task storage appears corrupted or missing. To recover:");
    println!();
    println!("1. Check if .scud/ directory exists:");
    println!("   {}", "ls -la .scud/".cyan());
    println!();
    println!("2. If missing, initialize SCUD:");
    println!("   {}", "scud init".cyan());
    println!();
    println!("3. If corrupted, check for backups:");
    println!("   {}", "ls -la .scud/tasks/*.bak".cyan());
    println!();
    println!("4. If no backups, you may need to recreate tasks:");
    println!(
        "   {}",
        "scud parse-prd <prd-file> --tag <epic-name>".cyan()
    );
    println!();
    println!("5. For manual recovery, task files are located at:");
    println!("   {}", ".scud/tasks/tasks.scg (or tasks.json)".dimmed());
    println!("   {}", ".scud/active-tag".dimmed());
    println!();
    println!(
        "{}",
        "If issues persist, consider consulting a high-context agent".yellow()
    );
    println!(
        "{}",
        "with full codebase access to inspect and repair the files.".yellow()
    );
}

/// Try to download a prebuilt rho-cli binary from GitHub releases.
fn try_install_rho_binary() -> bool {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let os_name = match os {
        "linux" => "linux",
        "macos" => "macos",
        _ => return false,
    };
    let arch_name = match arch {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => return false,
    };

    let asset = format!("rho-cli-{}-{}", os_name, arch_name);

    // Get latest release tag
    let tag_output = std::process::Command::new("curl")
        .args([
            "-sSf",
            "https://api.github.com/repos/pyrex41/rho/releases/latest",
        ])
        .output();

    let tag = match tag_output {
        Ok(output) => {
            let body = String::from_utf8_lossy(&output.stdout);
            body.lines()
                .find(|l| l.contains("\"tag_name\""))
                .and_then(|l| {
                    let after_key = &l[l.find("tag_name")? + 8..];
                    let q1 = after_key.find('"')? + 1;
                    let q2 = after_key[q1..].find('"')? + q1;
                    Some(after_key[q1..q2].to_string())
                })
        }
        Err(_) => None,
    };

    let tag = match tag {
        Some(t) => t,
        None => return false,
    };

    let url = format!(
        "https://github.com/pyrex41/rho/releases/download/{}/{}",
        tag, asset
    );

    // Install to ~/.local/bin or ~/.cargo/bin
    let install_dir = dirs::home_dir()
        .map(|h| {
            let local_bin = h.join(".local").join("bin");
            if local_bin.exists() {
                local_bin
            } else {
                h.join(".cargo").join("bin")
            }
        })
        .unwrap_or_else(|| PathBuf::from("/usr/local/bin"));

    let dest = install_dir.join("rho-cli");

    println!(
        "  Downloading rho-cli {} ({}/{})...",
        tag, os_name, arch_name
    );

    let status = std::process::Command::new("curl")
        .args(["-sSfL", "-o"])
        .arg(&dest)
        .arg(&url)
        .status();

    match status {
        Ok(s) if s.success() => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
            }
            println!("  Installed rho-cli to {}", dest.display());
            true
        }
        _ => false,
    }
}

/// Fallback: install rho-cli via cargo.
fn try_install_rho_cargo() -> bool {
    println!("  Trying cargo install rho-agent...");
    match std::process::Command::new("cargo")
        .args(["install", "rho-agent"])
        .status()
    {
        Ok(s) if s.success() => true,
        _ => false,
    }
}

pub fn scan_ext(project_root: Option<PathBuf>) -> Result<()> {
    use crate::extensions::loader::ExtensionManifest;
    use std::os::unix::fs::PermissionsExt;

    println!(
        "{}",
        "[EXPERIMENTAL] SCUD Doctor - Extension Scanner"
            .blue()
            .bold()
    );
    println!("{}", "=".repeat(60).blue());
    println!();

    let project_root = project_root.unwrap_or_else(|| std::env::current_dir().unwrap());
    let agents_dir = project_root.join(".scud").join("agents");

    println!("Scanning extensions in: {}", agents_dir.display());
    println!();

    let mut issues = Vec::new();
    let mut scanned_count = 0;

    // Check if agents directory exists
    if !agents_dir.exists() {
        println!(
            "{}",
            "No extensions directory found (.scud/agents/)".yellow()
        );
        println!("Extensions are automatically created when agents are configured.");
        return Ok(());
    }

    // Find all TOML files in the agents directory
    let entries = match std::fs::read_dir(&agents_dir) {
        Ok(entries) => entries,
        Err(e) => {
            issues.push(DiagnosticIssue {
                severity: Severity::Critical,
                epic_tag: "extensions".to_string(),
                task_id: None,
                message: format!("Cannot read extensions directory: {}", e),
                suggestion: r#"Check permissions on .scud/agents/ directory"#.to_string(),
            });
            return print_scan_results(&issues, scanned_count);
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                issues.push(DiagnosticIssue {
                    severity: Severity::Error,
                    epic_tag: "extensions".to_string(),
                    task_id: None,
                    message: format!("Error reading directory entry: {}", e),
                    suggestion: r#"Check directory permissions"#.to_string(),
                });
                continue;
            }
        };

        let path = entry.path();
        if !matches!(path.extension(), Some(ext) if ext == "toml") {
            continue;
        }

        scanned_count += 1;
        let filename = path.file_stem().unwrap_or_default().to_string_lossy();

        println!("Checking extension: {}", filename.cyan());

        // 1. Check manifest validity
        let manifest = match ExtensionManifest::from_file(&path) {
            Ok(m) => m,
            Err(e) => {
                issues.push(DiagnosticIssue {
                    severity: Severity::Critical,
                    epic_tag: "extensions".to_string(),
                    task_id: Some(filename.to_string()),
                    message: format!("Invalid manifest: {}", e),
                    suggestion: format!("Fix TOML syntax in {}", path.display()),
                });
                continue;
            }
        };

        // 2. Check file permissions
        match std::fs::metadata(&path) {
            Ok(metadata) => {
                let permissions = metadata.permissions();
                let mode = permissions.mode();

                // Check if readable by owner
                if mode & 0o400 == 0 {
                    issues.push(DiagnosticIssue {
                        severity: Severity::Error,
                        epic_tag: "extensions".to_string(),
                        task_id: Some(filename.to_string()),
                        message: format!("Extension file not readable: {}", path.display()),
                        suggestion: r#"Run: chmod +r <file>.toml"#.to_string(),
                    });
                }
            }
            Err(e) => {
                issues.push(DiagnosticIssue {
                    severity: Severity::Error,
                    epic_tag: "extensions".to_string(),
                    task_id: Some(filename.to_string()),
                    message: format!("Cannot access extension file: {}", e),
                    suggestion: r#"Check file permissions"#.to_string(),
                });
            }
        }

        // 3. Check required binaries (harnesses)
        if let Some(config) = manifest.config.get("harness") {
            if let Some(harness_str) = config.as_str() {
                // Try to parse the harness
                match Harness::parse(harness_str) {
                    Ok(harness) => {
                        // Try to find the harness binary
                        match find_harness_binary(harness) {
                            Ok(_) => {
                                // Binary found, good
                            }
                            Err(_) => {
                                issues.push(DiagnosticIssue {
                                    severity: Severity::Critical,
                                    epic_tag: "extensions".to_string(),
                                    task_id: Some(filename.to_string()),
                                    message: format!(
                                        r#"Required harness '{}' not found in PATH"#,
                                        harness_str
                                    ),
                                    suggestion: format!(r#"Install {} or check PATH"#, harness_str),
                                });
                            }
                        }
                    }
                    Err(e) => {
                        issues.push(DiagnosticIssue {
                            severity: Severity::Error,
                            epic_tag: "extensions".to_string(),
                            task_id: Some(filename.to_string()),
                            message: format!(r#"Invalid harness name '{}': {}"#, harness_str, e),
                            suggestion: r#"Use 'rho', 'claude', 'opencode', or 'cursor'"#
                                .to_string(),
                        });
                    }
                }
            }
        }

        // 4. Check for required dependencies
        for (dep_name, dep_version) in &manifest.dependencies {
            // For now, just warn about dependencies (we don't have a registry to check against)
            issues.push(DiagnosticIssue {
                severity: Severity::Warning,
                epic_tag: "extensions".to_string(),
                task_id: Some(filename.to_string()),
                message: format!(
                    r#"Extension has dependency '{}@{}' - validation not implemented"#,
                    dep_name, dep_version
                ),
                suggestion: r#"Ensure dependent extensions are installed"#.to_string(),
            });
        }

        // 5. Check for script files if they exist
        if let Some(script_path) = &manifest.extension.main {
            let script_full_path = agents_dir.join(script_path);
            if script_full_path.exists() {
                match std::fs::metadata(&script_full_path) {
                    Ok(metadata) => {
                        let permissions = metadata.permissions();
                        let mode = permissions.mode();

                        // Check if executable by owner
                        if mode & 0o100 == 0 {
                            issues.push(DiagnosticIssue {
                                severity: Severity::Warning,
                                epic_tag: "extensions".to_string(),
                                task_id: Some(filename.to_string()),
                                message: format!(
                                    "Script file not executable: {}",
                                    script_full_path.display()
                                ),
                                suggestion: r#"Run: chmod +x <script_file>.py"#.to_string(),
                            });
                        }
                    }
                    Err(e) => {
                        issues.push(DiagnosticIssue {
                            severity: Severity::Error,
                            epic_tag: "extensions".to_string(),
                            task_id: Some(filename.to_string()),
                            message: format!("Cannot access script file: {}", e),
                            suggestion: r#"Check script file permissions"#.to_string(),
                        });
                    }
                }
            } else {
                issues.push(DiagnosticIssue {
                    severity: Severity::Warning,
                    epic_tag: "extensions".to_string(),
                    task_id: Some(filename.to_string()),
                    message: format!(
                        "Referenced script file does not exist: {}",
                        script_full_path.display()
                    ),
                    suggestion: r#"Create the script file or update manifest"#.to_string(),
                });
            }
        }

        // Check tool scripts
        for tool in &manifest.tools {
            if let Some(script_path) = &tool.script {
                let script_full_path = agents_dir.join(script_path);
                if script_full_path.exists() {
                    match std::fs::metadata(&script_full_path) {
                        Ok(metadata) => {
                            let permissions = metadata.permissions();
                            let mode = permissions.mode();

                            if mode & 0o100 == 0 {
                                issues.push(DiagnosticIssue {
                                    severity: Severity::Warning,
                                    epic_tag: "extensions".to_string(),
                                    task_id: Some(format!("{}/{}", filename, tool.name)),
                                    message: format!(
                                        "Tool script not executable: {}",
                                        script_full_path.display()
                                    ),
                                    suggestion: r#"Run: chmod +x <script_file>.py"#.to_string(),
                                });
                            }
                        }
                        Err(e) => {
                            issues.push(DiagnosticIssue {
                                severity: Severity::Error,
                                epic_tag: "extensions".to_string(),
                                task_id: Some(format!("{}/{}", filename, tool.name)),
                                message: format!("Cannot access tool script: {}", e),
                                suggestion: r#"Check script file permissions"#.to_string(),
                            });
                        }
                    }
                } else {
                    issues.push(DiagnosticIssue {
                        severity: Severity::Error,
                        epic_tag: "extensions".to_string(),
                        task_id: Some(format!("{}/{}", filename, tool.name)),
                        message: format!(
                            "Tool script does not exist: {}",
                            script_full_path.display()
                        ),
                        suggestion: r#"Create the script file or update manifest"#.to_string(),
                    });
                }
            }
        }

        // Check event handler scripts
        for event in &manifest.events {
            if let Some(script_path) = &event.script {
                let script_full_path = agents_dir.join(script_path);
                if script_full_path.exists() {
                    match std::fs::metadata(&script_full_path) {
                        Ok(metadata) => {
                            let permissions = metadata.permissions();
                            let mode = permissions.mode();

                            if mode & 0o100 == 0 {
                                issues.push(DiagnosticIssue {
                                    severity: Severity::Warning,
                                    epic_tag: "extensions".to_string(),
                                    task_id: Some(format!("{}/{}", filename, event.event)),
                                    message: format!(
                                        "Event handler script not executable: {}",
                                        script_full_path.display()
                                    ),
                                    suggestion: r#"Run: chmod +x <script_file>.py"#.to_string(),
                                });
                            }
                        }
                        Err(e) => {
                            issues.push(DiagnosticIssue {
                                severity: Severity::Error,
                                epic_tag: "extensions".to_string(),
                                task_id: Some(format!("{}/{}", filename, event.event)),
                                message: format!("Cannot access event handler script: {}", e),
                                suggestion: r#"Check script file permissions"#.to_string(),
                            });
                        }
                    }
                } else {
                    issues.push(DiagnosticIssue {
                        severity: Severity::Error,
                        epic_tag: "extensions".to_string(),
                        task_id: Some(format!("{}/{}", filename, event.event)),
                        message: format!(
                            "Event handler script does not exist: {}",
                            script_full_path.display()
                        ),
                        suggestion: r#"Create the script file or update manifest"#.to_string(),
                    });
                }
            }
        }
    }

    print_scan_results(&issues, scanned_count)
}

fn print_scan_results(issues: &[DiagnosticIssue], scanned_count: usize) -> Result<()> {
    println!("Scanned {} extension(s)", scanned_count);
    println!();

    if issues.is_empty() {
        println!(
            "{}",
            "\u{2713} All extensions are valid and properly configured!"
                .green()
                .bold()
        );
        return Ok(());
    }

    let critical_count = issues
        .iter()
        .filter(|i| i.severity == Severity::Critical)
        .count();
    let error_count = issues
        .iter()
        .filter(|i| i.severity == Severity::Error)
        .count();
    let warning_count = issues
        .iter()
        .filter(|i| i.severity == Severity::Warning)
        .count();

    // Print issues by severity
    for severity in &[Severity::Critical, Severity::Error, Severity::Warning] {
        let severity_issues: Vec<_> = issues.iter().filter(|i| i.severity == *severity).collect();

        if severity_issues.is_empty() {
            continue;
        }

        let title = match severity {
            Severity::Critical => "CRITICAL ISSUES".red().bold(),
            Severity::Error => "ERRORS".red().bold(),
            Severity::Warning => "WARNINGS".yellow().bold(),
        };

        println!("{}", title);
        println!("{}", "-".repeat(40));

        for issue in severity_issues {
            let icon = match severity {
                Severity::Critical => "\u{2717}".red(),
                Severity::Error => "\u{2717}".red(),
                Severity::Warning => "\u{26A0}".yellow(),
            };

            println!("  {} {}", icon, issue.message);

            if let Some(ref task_id) = issue.task_id {
                println!("    Extension: {}", task_id.cyan());
            }

            println!("    {}", format!("\u{2192} {}", issue.suggestion).dimmed());
            println!();
        }
    }

    // Summary
    println!("{}", "Summary".blue().bold());
    println!("{}", "-".repeat(40).blue());
    println!(
        "  Critical: {}  Errors: {}  Warnings: {}",
        critical_count.to_string().red(),
        error_count.to_string().red(),
        warning_count.to_string().yellow()
    );

    if critical_count > 0 {
        println!();
        println!(
            "{}",
            "Critical issues prevent extensions from functioning. Fix them first.".red()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::phase::Phase;
    use crate::models::task::Task;

    #[test]
    fn test_diagnostic_results_has_issues() {
        let empty = DiagnosticResults::default();
        assert!(!empty.has_issues());

        let mut with_orphan = DiagnosticResults::default();
        with_orphan
            .orphan_in_progress
            .push(("epic".to_string(), "task".to_string()));
        assert!(with_orphan.has_issues());
    }

    #[test]
    fn test_diagnostic_results_counts() {
        let mut results = DiagnosticResults::default();

        // Add orphan in-progress (warnings)
        results
            .orphan_in_progress
            .push(("epic".to_string(), "task1".to_string()));
        results
            .orphan_in_progress
            .push(("epic".to_string(), "task2".to_string()));

        // Add blocked by cancelled (errors)
        results.blocked_by_cancelled.push((
            "epic".to_string(),
            "task3".to_string(),
            "dep1".to_string(),
        ));

        // Add corrupt files (critical)
        results
            .corrupt_files
            .push("tasks.json: parse error".to_string());

        assert_eq!(results.warning_count(), 2);
        assert_eq!(results.error_count(), 1);
        assert_eq!(results.critical_count(), 1);
    }

    #[test]
    fn test_severity_as_str() {
        assert_eq!(Severity::Warning.as_str(), "WARNING");
        assert_eq!(Severity::Error.as_str(), "ERROR");
        assert_eq!(Severity::Critical.as_str(), "CRITICAL");
    }

    fn create_test_phase_with_issues() -> Phase {
        let mut phase = Phase::new("test-phase".to_string());

        // Task 1: Done
        let mut task1 = Task::new("1".to_string(), "Task 1".to_string(), "Desc".to_string());
        task1.set_status(TaskStatus::Done);
        phase.add_task(task1);

        // Task 2: Cancelled (will block task 3)
        let mut task2 = Task::new("2".to_string(), "Task 2".to_string(), "Desc".to_string());
        task2.set_status(TaskStatus::Cancelled);
        phase.add_task(task2);

        // Task 3: Pending, depends on cancelled task 2
        let mut task3 = Task::new("3".to_string(), "Task 3".to_string(), "Desc".to_string());
        task3.dependencies = vec!["2".to_string()];
        phase.add_task(task3);

        // Task 4: Pending, depends on non-existent task
        let mut task4 = Task::new("4".to_string(), "Task 4".to_string(), "Desc".to_string());
        task4.dependencies = vec!["nonexistent".to_string()];
        phase.add_task(task4);

        phase
    }

    #[test]
    fn test_detect_cancelled_dependency() {
        let phase = create_test_phase_with_issues();

        let task3 = phase.get_task("3").unwrap();
        let mut found_cancelled_dep = false;

        for dep_id in &task3.dependencies {
            if let Some(dep_task) = phase.get_task(dep_id) {
                if dep_task.status == TaskStatus::Cancelled {
                    found_cancelled_dep = true;
                }
            }
        }

        assert!(found_cancelled_dep);
    }

    #[test]
    fn test_detect_missing_dependency() {
        let phase = create_test_phase_with_issues();
        let all_task_ids: std::collections::HashSet<_> =
            phase.tasks.iter().map(|t| t.id.clone()).collect();
        // Use all_task_ids to check for missing dependencies
        let _task_count = all_task_ids.len();

        let task4 = phase.get_task("4").unwrap();
        let mut found_missing_dep = false;

        for dep_id in &task4.dependencies {
            if !all_task_ids.contains(dep_id) {
                found_missing_dep = true;
            }
        }

        assert!(found_missing_dep);
    }
}
