//! Handoff prompt generation for Ralph Wiggum mode
//!
//! After each wave, generates a "handoff" prompt that carries context
//! from the completed wave into the next wave. This maintains continuity
//! and helps subsequent agents understand what was done.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use super::review::ReviewResult;
use super::session::WaveState;

/// Handoff context passed between waves
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Handoff {
    /// Wave number this handoff is from
    pub from_wave: usize,
    /// Context summary for next wave
    pub context: String,
    /// Key decisions made
    pub decisions: Vec<String>,
    /// Files that were modified
    pub files_touched: Vec<String>,
    /// Patterns established
    pub patterns: Vec<String>,
    /// Warnings or gotchas for next wave
    pub warnings: Vec<String>,
    /// Timestamp
    pub created_at: String,
}

impl Handoff {
    pub fn new(from_wave: usize) -> Self {
        Self {
            from_wave,
            context: String::new(),
            decisions: Vec::new(),
            files_touched: Vec::new(),
            patterns: Vec::new(),
            warnings: Vec::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Load handoff from disk
    pub fn load(project_root: Option<&PathBuf>, tag: &str) -> Result<Self> {
        let file = handoff_file(project_root, tag);
        let json = fs::read_to_string(&file)?;
        let handoff: Handoff = serde_json::from_str(&json)?;
        Ok(handoff)
    }

    /// Save handoff to disk
    pub fn save(&self, project_root: Option<&PathBuf>, tag: &str) -> Result<()> {
        let dir = ralph_dir(project_root);
        fs::create_dir_all(&dir)?;

        let file = handoff_file(project_root, tag);
        let json = serde_json::to_string_pretty(self)?;
        fs::write(file, json)?;

        Ok(())
    }
}

/// Get the ralph directory
fn ralph_dir(project_root: Option<&PathBuf>) -> PathBuf {
    let root = project_root
        .cloned()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    root.join(".scud").join("ralph")
}

/// Get the handoff file path
fn handoff_file(project_root: Option<&PathBuf>, tag: &str) -> PathBuf {
    ralph_dir(project_root).join(format!("{}-handoff.json", tag))
}

/// Generate a handoff prompt from review results
pub fn generate_handoff(
    review: &ReviewResult,
    _wave: &WaveState,
    wave_number: usize,
) -> Handoff {
    let mut handoff = Handoff::new(wave_number);

    // Collect files touched
    for tr in &review.task_reviews {
        handoff.files_touched.extend(tr.files_changed.clone());
    }
    handoff.files_touched.sort();
    handoff.files_touched.dedup();

    // Extract warnings from issues
    handoff.warnings = review.issues_found.clone();

    // Build context summary
    let tasks_completed: Vec<String> = review.task_reviews
        .iter()
        .map(|tr| format!("{}: {}", tr.task_id, tr.task_title))
        .collect();

    let mut context_parts: Vec<String> = Vec::new();

    // Summary of what was done
    if !tasks_completed.is_empty() {
        context_parts.push(format!(
            "Wave {} completed {} task(s):\n{}",
            wave_number,
            tasks_completed.len(),
            tasks_completed.iter()
                .map(|t| format!("  - {}", t))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    // Files modified
    if !handoff.files_touched.is_empty() {
        let file_summary = if handoff.files_touched.len() <= 5 {
            handoff.files_touched.join(", ")
        } else {
            format!(
                "{} and {} more",
                handoff.files_touched[..5].join(", "),
                handoff.files_touched.len() - 5
            )
        };
        context_parts.push(format!("Files modified: {}", file_summary));
    }

    // Issues and recommendations
    if !review.issues_found.is_empty() {
        context_parts.push(format!(
            "Issues noted: {}",
            review.issues_found.join("; ")
        ));
    }

    if !review.recommendations.is_empty() {
        context_parts.push(format!(
            "Recommendations: {}",
            review.recommendations.join("; ")
        ));
    }

    // Overall summary
    if !review.summary.is_empty() {
        context_parts.push(format!("Review summary: {}", review.summary));
    }

    handoff.context = context_parts.join("\n\n");

    // Extract any patterns mentioned in recommendations
    handoff.patterns = review.recommendations
        .iter()
        .filter(|r| r.to_lowercase().contains("pattern") || r.to_lowercase().contains("convention"))
        .cloned()
        .collect();

    handoff
}

/// Generate a minimal handoff when review is disabled
pub fn generate_minimal_handoff(
    wave: &WaveState,
    wave_number: usize,
) -> Handoff {
    let mut handoff = Handoff::new(wave_number);

    let task_ids = wave.all_task_ids();

    handoff.context = format!(
        "Wave {} completed {} task(s): {}",
        wave_number,
        task_ids.len(),
        task_ids.join(", ")
    );

    handoff
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::ralph::review::TaskReview;

    #[test]
    fn test_handoff_new() {
        let handoff = Handoff::new(1);
        assert_eq!(handoff.from_wave, 1);
        assert!(handoff.context.is_empty());
    }

    #[test]
    fn test_generate_handoff() {
        let review = ReviewResult {
            wave_number: 1,
            tasks_reviewed: 2,
            issues_found: vec!["Missing error handling".to_string()],
            fixes_applied: Vec::new(),
            recommendations: vec!["Use consistent error pattern".to_string()],
            task_reviews: vec![
                TaskReview {
                    task_id: "task:1".to_string(),
                    task_title: "Add auth".to_string(),
                    files_changed: vec!["src/auth.rs".to_string()],
                    lines_added: 50,
                    lines_removed: 10,
                    issues: Vec::new(),
                    passed: true,
                    notes: String::new(),
                },
            ],
            summary: "Good progress".to_string(),
            reviewed_at: chrono::Utc::now().to_rfc3339(),
        };

        let wave = WaveState::new(1);
        let handoff = generate_handoff(&review, &wave, 1);

        assert_eq!(handoff.from_wave, 1);
        assert!(handoff.context.contains("Wave 1"));
        assert!(handoff.context.contains("task:1"));
        assert_eq!(handoff.files_touched, vec!["src/auth.rs"]);
        assert!(handoff.warnings.contains(&"Missing error handling".to_string()));
    }

    #[test]
    fn test_save_and_load_handoff() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        let mut handoff = Handoff::new(1);
        handoff.context = "Test context".to_string();
        handoff.warnings.push("Test warning".to_string());

        handoff.save(Some(&project_root), "test-tag").unwrap();

        let loaded = Handoff::load(Some(&project_root), "test-tag").unwrap();
        assert_eq!(loaded.from_wave, 1);
        assert_eq!(loaded.context, "Test context");
        assert!(loaded.warnings.contains(&"Test warning".to_string()));
    }
}
