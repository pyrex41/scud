use regex::Regex;

/// Attempt to parse token usage from captured agent output
///
/// Different CLIs report usage differently:
/// - Claude Code: May print "Tokens: X input, Y output"
/// - OpenCode: May print cost/token summaries
pub fn estimate_tokens_from_output(output: &str) -> Option<TokenEstimate> {
    // Claude Code pattern (hypothetical - check actual output)
    let claude_re = Regex::new(r"(?i)tokens?:\s*(\d+)\s*input.*?(\d+)\s*output").ok()?;
    if let Some(caps) = claude_re.captures(output) {
        return Some(TokenEstimate {
            input: caps.get(1)?.as_str().parse().ok()?,
            output: caps.get(2)?.as_str().parse().ok()?,
        });
    }

    // OpenCode pattern (hypothetical)
    let opencode_re = Regex::new(r"(?i)usage:\s*(\d+)\s*/\s*(\d+)").ok()?;
    if let Some(caps) = opencode_re.captures(output) {
        return Some(TokenEstimate {
            input: caps.get(1)?.as_str().parse().ok()?,
            output: caps.get(2)?.as_str().parse().ok()?,
        });
    }

    None
}

pub struct TokenEstimate {
    pub input: u64,
    pub output: u64,
}

/// Estimate cost based on model and token counts
pub fn estimate_cost(model: &str, tokens: &TokenEstimate) -> f64 {
    // Approximate pricing (update as needed)
    let (input_rate, output_rate) = match model {
        m if m.contains("opus") => (15.0 / 1_000_000.0, 75.0 / 1_000_000.0),
        m if m.contains("sonnet") => (3.0 / 1_000_000.0, 15.0 / 1_000_000.0),
        m if m.contains("haiku") => (0.25 / 1_000_000.0, 1.25 / 1_000_000.0),
        m if m.contains("grok") => (2.0 / 1_000_000.0, 10.0 / 1_000_000.0),
        _ => (5.0 / 1_000_000.0, 15.0 / 1_000_000.0), // Default estimate
    };

    (tokens.input as f64 * input_rate) + (tokens.output as f64 * output_rate)
}
