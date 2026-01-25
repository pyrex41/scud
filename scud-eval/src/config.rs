use crate::metrics::ExecutionMode;
use anyhow::Result;

#[derive(Clone, Debug)]
pub struct EvalConfig {
    pub mode: ExecutionMode,
    pub taskset_name: String,
    pub harness: String,
    pub model: Option<String>,
}

pub fn parse_mode_config(
    mode_str: &str,
    tasks: &str,
    harness: &str,
    model: Option<String>,
) -> Result<EvalConfig> {
    let mode = match mode_str {
        m if m.starts_with("swarm-") => {
            let size: usize = m
                .strip_prefix("swarm-")
                .unwrap_or(m)
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid swarm size"))?;
            ExecutionMode::Swarm { round_size: size }
        }
        "ralph" => ExecutionMode::Ralph,
        "claude-direct" => ExecutionMode::ClaudeDirect,
        _ => anyhow::bail!("Unknown mode: {}", mode_str),
    };
    Ok(EvalConfig {
        mode,
        taskset_name: tasks.to_string(),
        harness: harness.to_string(),
        model,
    })
}
