//! Failure attribution using git blame
//!
//! Maps validation errors to specific tasks by:
//! 1. Parsing error output for file:line references
//! 2. Using git blame to find which commit changed each line
//! 3. Extracting task IDs from commit messages ([TASK-ID] prefix)

use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

/// Result of attributing a failure to tasks
#[derive(Debug, Clone)]
pub struct Attribution {
    /// Task IDs that likely caused the failure
    pub responsible_tasks: Vec<String>,
    /// Task IDs that are probably not responsible
    pub cleared_tasks: Vec<String>,
    /// Whether attribution was definitive or uncertain
    pub confidence: AttributionConfidence,
    /// Raw evidence used for attribution
    pub evidence: Vec<AttributionEvidence>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttributionConfidence {
    /// Clear single task responsible
    High,
    /// Multiple tasks may be responsible
    Medium,
    /// Could not determine - all tasks suspect
    Low,
}

#[derive(Debug, Clone)]
pub struct AttributionEvidence {
    pub file: String,
    pub line: Option<u32>,
    pub task_id: Option<String>,
    pub commit_sha: Option<String>,
    pub error_snippet: String,
}

/// Parse error output for file:line references
pub fn parse_error_locations(stderr: &str, stdout: &str) -> Vec<(String, Option<u32>)> {
    let mut locations = Vec::new();
    let combined = format!("{}\n{}", stderr, stdout);

    // Common patterns:
    // Rust: --> src/main.rs:42:5
    // TypeScript: src/index.ts(10,5): error
    // Go: ./main.go:15:3:
    // Python: File "script.py", line 10
    // Generic: filename:line or filename:line:col

    let patterns = [
        r"(?:-->|error\[.*?\]:)\s+([^:\s]+):(\d+)",  // Rust
        r"([^\s(]+)\((\d+),\d+\):",                   // TypeScript
        r"([^\s:]+):(\d+):\d+:",                      // Go/generic
        r#"File "([^"]+)", line (\d+)"#,              // Python
        r"([^\s:]+):(\d+)",                           // Generic fallback
    ];

    for pattern in patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            for cap in re.captures_iter(&combined) {
                if let (Some(file), Some(line)) = (cap.get(1), cap.get(2)) {
                    let file_str = file.as_str().to_string();
                    let line_num = line.as_str().parse::<u32>().ok();
                    if !locations.iter().any(|(f, _)| f == &file_str) {
                        locations.push((file_str, line_num));
                    }
                }
            }
        }
    }

    locations
}

/// Get task ID from a commit message (looks for [TASK-ID] prefix)
pub fn extract_task_id_from_commit(message: &str) -> Option<String> {
    let re = regex::Regex::new(r"\[([^\]]+)\]").ok()?;
    re.captures(message)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
}

/// Use git blame to find which task changed a specific line
pub fn blame_line(working_dir: &Path, file: &str, line: u32) -> Result<Option<String>> {
    let output = Command::new("git")
        .current_dir(working_dir)
        .args(["blame", "-L", &format!("{},{}", line, line), "--porcelain", file])
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let blame_output = String::from_utf8_lossy(&output.stdout);

    // Look for "summary" line in porcelain output
    for blame_line in blame_output.lines() {
        if blame_line.starts_with("summary ") {
            let message = blame_line.strip_prefix("summary ").unwrap_or("");
            return Ok(extract_task_id_from_commit(message));
        }
    }

    Ok(None)
}

/// Get all commits in range that match task ID pattern
pub fn get_task_commits(
    working_dir: &Path,
    start_commit: Option<&str>,
) -> Result<HashMap<String, Vec<String>>> {
    let range = match start_commit {
        Some(commit) => format!("{}..HEAD", commit),
        None => "HEAD~10..HEAD".to_string(),
    };

    let output = Command::new("git")
        .current_dir(working_dir)
        .args(["log", "--format=%H %s", &range])
        .output()?;

    let mut task_commits: HashMap<String, Vec<String>> = HashMap::new();

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() == 2 {
            let sha = parts[0].to_string();
            let message = parts[1];
            if let Some(task_id) = extract_task_id_from_commit(message) {
                task_commits.entry(task_id).or_default().push(sha);
            }
        }
    }

    Ok(task_commits)
}

/// Get files changed by a specific task (via its commits)
pub fn get_task_changed_files(
    working_dir: &Path,
    task_id: &str,
    start_commit: Option<&str>,
) -> Result<HashSet<String>> {
    let task_commits = get_task_commits(working_dir, start_commit)?;
    let mut files = HashSet::new();

    if let Some(commits) = task_commits.get(task_id) {
        for sha in commits {
            let output = Command::new("git")
                .current_dir(working_dir)
                .args(["diff-tree", "--no-commit-id", "--name-only", "-r", sha])
                .output()?;

            for file in String::from_utf8_lossy(&output.stdout).lines() {
                files.insert(file.to_string());
            }
        }
    }

    Ok(files)
}

/// Main attribution function - attributes validation failure to tasks
pub fn attribute_failure(
    working_dir: &Path,
    stderr: &str,
    stdout: &str,
    wave_tasks: &[String],
    start_commit: Option<&str>,
) -> Result<Attribution> {
    let mut evidence = Vec::new();
    let mut responsible: HashSet<String> = HashSet::new();

    // Parse error locations
    let locations = parse_error_locations(stderr, stdout);

    // Try to blame each location
    for (file, line_opt) in &locations {
        let mut ev = AttributionEvidence {
            file: file.clone(),
            line: *line_opt,
            task_id: None,
            commit_sha: None,
            error_snippet: String::new(),
        };

        if let Some(line) = line_opt {
            if let Ok(Some(task_id)) = blame_line(working_dir, file, *line) {
                if wave_tasks.contains(&task_id) {
                    responsible.insert(task_id.clone());
                    ev.task_id = Some(task_id);
                }
            }
        }

        evidence.push(ev);
    }

    // If no direct attribution, check which tasks touched error files
    if responsible.is_empty() && !locations.is_empty() {
        let error_files: HashSet<String> = locations.iter().map(|(f, _)| f.clone()).collect();

        for task_id in wave_tasks {
            if let Ok(task_files) = get_task_changed_files(working_dir, task_id, start_commit) {
                if !task_files.is_disjoint(&error_files) {
                    responsible.insert(task_id.clone());
                }
            }
        }
    }

    let confidence = if responsible.len() == 1 {
        AttributionConfidence::High
    } else if !responsible.is_empty() {
        AttributionConfidence::Medium
    } else {
        // Could not attribute - all tasks suspect
        responsible.extend(wave_tasks.iter().cloned());
        AttributionConfidence::Low
    };

    let cleared: Vec<String> = wave_tasks
        .iter()
        .filter(|t| !responsible.contains(*t))
        .cloned()
        .collect();

    Ok(Attribution {
        responsible_tasks: responsible.into_iter().collect(),
        cleared_tasks: cleared,
        confidence,
        evidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_task_id_from_commit() {
        assert_eq!(
            extract_task_id_from_commit("[auth:1] Add login endpoint"),
            Some("auth:1".to_string())
        );
        assert_eq!(
            extract_task_id_from_commit("[TASK-123] Fix bug"),
            Some("TASK-123".to_string())
        );
        assert_eq!(
            extract_task_id_from_commit("No task ID here"),
            None
        );
    }

    #[test]
    fn test_parse_error_locations_rust() {
        let stderr = r#"
error[E0308]: mismatched types
 --> src/main.rs:42:5
  |
42 |     let x: i32 = "hello";
  |                  ^^^^^^^ expected `i32`, found `&str`
"#;
        let locations = parse_error_locations(stderr, "");
        assert!(!locations.is_empty());
        assert!(locations.iter().any(|(f, l)| f == "src/main.rs" && *l == Some(42)));
    }

    #[test]
    fn test_parse_error_locations_python() {
        let stderr = r#"
Traceback (most recent call last):
  File "script.py", line 10, in <module>
    raise ValueError("test")
ValueError: test
"#;
        let locations = parse_error_locations(stderr, "");
        assert!(!locations.is_empty());
        assert!(locations.iter().any(|(f, l)| f == "script.py" && *l == Some(10)));
    }

    #[test]
    fn test_parse_error_locations_go() {
        let stderr = "./main.go:15:3: undefined: foo\n";
        let locations = parse_error_locations(stderr, "");
        assert!(!locations.is_empty());
        assert!(locations.iter().any(|(f, l)| f == "./main.go" && *l == Some(15)));
    }

    #[test]
    fn test_parse_error_locations_empty() {
        let locations = parse_error_locations("", "");
        assert!(locations.is_empty());
    }

    #[test]
    fn test_attribution_confidence() {
        assert_eq!(AttributionConfidence::High, AttributionConfidence::High);
        assert_ne!(AttributionConfidence::High, AttributionConfidence::Low);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::TempDir;
    use std::process::Command;

    /// Test with a real git repo to verify blame functionality
    #[test]
    fn test_blame_in_real_git_repo() {
        let temp = TempDir::new().unwrap();
        let repo_dir = temp.path();

        // Initialize git repo
        Command::new("git")
            .current_dir(repo_dir)
            .args(["init"])
            .output()
            .unwrap();

        // Configure git user for commits
        Command::new("git")
            .current_dir(repo_dir)
            .args(["config", "user.email", "test@test.com"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(repo_dir)
            .args(["config", "user.name", "Test"])
            .output()
            .unwrap();

        // Create a file and commit with task ID
        std::fs::write(repo_dir.join("test.rs"), "fn main() {}\n").unwrap();
        Command::new("git")
            .current_dir(repo_dir)
            .args(["add", "test.rs"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(repo_dir)
            .args(["commit", "-m", "[auth:1] Initial commit"])
            .output()
            .unwrap();

        // Now test blame_line
        let result = blame_line(repo_dir, "test.rs", 1).unwrap();
        assert_eq!(result, Some("auth:1".to_string()));
    }

    #[test]
    fn test_get_task_commits() {
        let temp = TempDir::new().unwrap();
        let repo_dir = temp.path();

        // Initialize git repo
        Command::new("git")
            .current_dir(repo_dir)
            .args(["init"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(repo_dir)
            .args(["config", "user.email", "test@test.com"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(repo_dir)
            .args(["config", "user.name", "Test"])
            .output()
            .unwrap();

        // Create initial commit (baseline, no task ID)
        std::fs::write(repo_dir.join("init.txt"), "init").unwrap();
        Command::new("git").current_dir(repo_dir).args(["add", "."]).output().unwrap();
        Command::new("git").current_dir(repo_dir).args(["commit", "-m", "Initial commit"]).output().unwrap();

        // Get SHA of initial commit to use as range start
        let init_sha = Command::new("git")
            .current_dir(repo_dir)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        let init_sha = String::from_utf8_lossy(&init_sha.stdout).trim().to_string();

        // Create commits with different task IDs
        std::fs::write(repo_dir.join("a.txt"), "a").unwrap();
        Command::new("git").current_dir(repo_dir).args(["add", "."]).output().unwrap();
        Command::new("git").current_dir(repo_dir).args(["commit", "-m", "[task:1] First"]).output().unwrap();

        std::fs::write(repo_dir.join("b.txt"), "b").unwrap();
        Command::new("git").current_dir(repo_dir).args(["add", "."]).output().unwrap();
        Command::new("git").current_dir(repo_dir).args(["commit", "-m", "[task:2] Second"]).output().unwrap();

        std::fs::write(repo_dir.join("c.txt"), "c").unwrap();
        Command::new("git").current_dir(repo_dir).args(["add", "."]).output().unwrap();
        Command::new("git").current_dir(repo_dir).args(["commit", "-m", "[task:1] More for task 1"]).output().unwrap();

        // Now use the init commit as range start (excludes init, includes task commits)
        let task_commits = get_task_commits(repo_dir, Some(&init_sha)).unwrap();

        // task:1 should have 2 commits
        assert_eq!(task_commits.get("task:1").map(|v| v.len()), Some(2));
        // task:2 should have 1 commit
        assert_eq!(task_commits.get("task:2").map(|v| v.len()), Some(1));
    }

    #[test]
    fn test_attribute_failure_with_git_repo() {
        let temp = TempDir::new().unwrap();
        let repo_dir = temp.path();

        // Initialize git repo
        Command::new("git").current_dir(repo_dir).args(["init"]).output().unwrap();
        Command::new("git").current_dir(repo_dir).args(["config", "user.email", "test@test.com"]).output().unwrap();
        Command::new("git").current_dir(repo_dir).args(["config", "user.name", "Test"]).output().unwrap();

        // Create src directory FIRST, then write the file
        std::fs::create_dir_all(repo_dir.join("src")).unwrap();
        std::fs::write(repo_dir.join("src/main.rs"), "fn main() {\n    let x: i32 = \"bad\";\n}\n").unwrap();
        Command::new("git").current_dir(repo_dir).args(["add", "."]).output().unwrap();
        Command::new("git").current_dir(repo_dir).args(["commit", "-m", "[api:1] Add main file"]).output().unwrap();

        // Simulate a compilation error pointing to line 2
        let stderr = r#"error[E0308]: mismatched types
 --> src/main.rs:2:18
  |
2 |     let x: i32 = "bad";
  |                  ^^^^^ expected `i32`, found `&str`
"#;

        let wave_tasks = vec!["api:1".to_string(), "api:2".to_string()];
        let attribution = attribute_failure(repo_dir, stderr, "", &wave_tasks, None).unwrap();

        // api:1 should be responsible (it created line 2)
        assert!(attribution.responsible_tasks.contains(&"api:1".to_string()));
        // api:2 should be cleared
        assert!(attribution.cleared_tasks.contains(&"api:2".to_string()));
        // High confidence since we found a direct blame match
        assert_eq!(attribution.confidence, AttributionConfidence::High);
    }
}
