use anyhow::Result;
use std::path::PathBuf;

/// Storage location: ~/.scud-eval/
pub fn eval_home() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".scud-eval")
}

pub fn runs_dir() -> PathBuf {
    eval_home().join("runs")
}

pub fn tasksets_dir() -> PathBuf {
    eval_home().join("tasksets")
}

pub fn run_dir(run_id: &str) -> PathBuf {
    runs_dir().join(run_id)
}

/// Save eval run results
pub fn save_run(metrics: &crate::metrics::EvalRunMetrics) -> Result<PathBuf> {
    let dir = run_dir(&metrics.run_id);
    std::fs::create_dir_all(&dir)?;

    let path = dir.join("metrics.json");
    let json = serde_json::to_string_pretty(metrics)?;
    std::fs::write(&path, json)?;

    Ok(path)
}

/// Load eval run results
pub fn load_run(run_id: &str) -> Result<crate::metrics::EvalRunMetrics> {
    let path = run_dir(run_id).join("metrics.json");
    let json = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}

/// List all run IDs
pub fn list_runs() -> Result<Vec<String>> {
    let dir = runs_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut runs = vec![];
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                runs.push(name.to_string());
            }
        }
    }
    runs.sort();
    Ok(runs)
}
