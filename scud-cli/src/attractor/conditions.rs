//! Condition expression parser and evaluator.
//!
//! Minimal boolean language for edge conditions:
//! - `key=value` — equality check
//! - `key!=value` — inequality check
//! - `expr && expr` — conjunction
//!
//! Keys can be:
//! - `outcome` — matches the outcome status string
//! - `preferred_label` — matches the preferred label from outcome
//! - `context.key` — looks up key in the execution context
//! - Unqualified keys fall back to context lookup

use std::collections::HashMap;

use super::outcome::Outcome;

/// A parsed condition expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    /// Always true (empty condition).
    Always,
    /// key = value
    Eq(String, String),
    /// key != value
    Neq(String, String),
    /// All conditions must be true.
    And(Vec<Condition>),
}

/// Parse a condition string into a Condition.
pub fn parse_condition(input: &str) -> Condition {
    let input = input.trim();
    if input.is_empty() {
        return Condition::Always;
    }

    // Split on &&
    let parts: Vec<&str> = input.split("&&").collect();
    if parts.len() > 1 {
        let conditions: Vec<Condition> = parts
            .into_iter()
            .map(|p| parse_single_condition(p.trim()))
            .collect();
        return Condition::And(conditions);
    }

    parse_single_condition(input)
}

fn parse_single_condition(input: &str) -> Condition {
    let input = input.trim();
    if input.is_empty() {
        return Condition::Always;
    }

    // Check for != first (before =)
    if let Some(pos) = input.find("!=") {
        let key = input[..pos].trim().to_string();
        let value = input[pos + 2..].trim().to_string();
        return Condition::Neq(key, value);
    }

    // Check for =
    if let Some(pos) = input.find('=') {
        let key = input[..pos].trim().to_string();
        let value = input[pos + 1..].trim().to_string();
        return Condition::Eq(key, value);
    }

    // Bare word — treat as key=true
    Condition::Eq(input.to_string(), "true".to_string())
}

/// Evaluate a condition against an outcome and context.
pub fn evaluate_condition(
    condition: &Condition,
    outcome: &Outcome,
    context: &HashMap<String, serde_json::Value>,
) -> bool {
    match condition {
        Condition::Always => true,
        Condition::Eq(key, value) => resolve_value(key, outcome, context) == *value,
        Condition::Neq(key, value) => resolve_value(key, outcome, context) != *value,
        Condition::And(conditions) => conditions
            .iter()
            .all(|c| evaluate_condition(c, outcome, context)),
    }
}

/// Resolve a key to a string value from the outcome or context.
fn resolve_value(
    key: &str,
    outcome: &Outcome,
    context: &HashMap<String, serde_json::Value>,
) -> String {
    match key {
        "outcome" => outcome.status.as_str().to_string(),
        "preferred_label" => outcome
            .preferred_label
            .clone()
            .unwrap_or_default(),
        _ => {
            // Try context.key prefix
            let ctx_key = if key.starts_with("context.") {
                &key[8..]
            } else {
                key
            };
            context
                .get(ctx_key)
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Number(n) => n.to_string(),
                    other => other.to_string(),
                })
                .unwrap_or_default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attractor::outcome::Outcome;

    #[test]
    fn test_parse_empty() {
        assert_eq!(parse_condition(""), Condition::Always);
    }

    #[test]
    fn test_parse_eq() {
        assert_eq!(
            parse_condition("outcome=success"),
            Condition::Eq("outcome".into(), "success".into())
        );
    }

    #[test]
    fn test_parse_neq() {
        assert_eq!(
            parse_condition("outcome!=failure"),
            Condition::Neq("outcome".into(), "failure".into())
        );
    }

    #[test]
    fn test_parse_and() {
        let cond = parse_condition("outcome=success && context.approved=true");
        match cond {
            Condition::And(parts) => {
                assert_eq!(parts.len(), 2);
                assert_eq!(
                    parts[0],
                    Condition::Eq("outcome".into(), "success".into())
                );
                assert_eq!(
                    parts[1],
                    Condition::Eq("context.approved".into(), "true".into())
                );
            }
            _ => panic!("Expected And condition"),
        }
    }

    #[test]
    fn test_evaluate_outcome_eq() {
        let outcome = Outcome::success();
        let ctx = HashMap::new();
        let cond = parse_condition("outcome=success");
        assert!(evaluate_condition(&cond, &outcome, &ctx));
    }

    #[test]
    fn test_evaluate_outcome_neq() {
        let outcome = Outcome::success();
        let ctx = HashMap::new();
        let cond = parse_condition("outcome!=failure");
        assert!(evaluate_condition(&cond, &outcome, &ctx));
    }

    #[test]
    fn test_evaluate_preferred_label() {
        let outcome = Outcome::success_with_label("approve");
        let ctx = HashMap::new();
        let cond = parse_condition("preferred_label=approve");
        assert!(evaluate_condition(&cond, &outcome, &ctx));
    }

    #[test]
    fn test_evaluate_context_value() {
        let outcome = Outcome::success();
        let mut ctx = HashMap::new();
        ctx.insert("test_passed".into(), serde_json::json!("true"));
        let cond = parse_condition("test_passed=true");
        assert!(evaluate_condition(&cond, &outcome, &ctx));
    }

    #[test]
    fn test_evaluate_context_prefix() {
        let outcome = Outcome::success();
        let mut ctx = HashMap::new();
        ctx.insert("flag".into(), serde_json::json!("yes"));
        let cond = parse_condition("context.flag=yes");
        assert!(evaluate_condition(&cond, &outcome, &ctx));
    }

    #[test]
    fn test_evaluate_and_all_true() {
        let outcome = Outcome::success();
        let mut ctx = HashMap::new();
        ctx.insert("ready".into(), serde_json::json!("true"));
        let cond = parse_condition("outcome=success && ready=true");
        assert!(evaluate_condition(&cond, &outcome, &ctx));
    }

    #[test]
    fn test_evaluate_and_one_false() {
        let outcome = Outcome::success();
        let ctx = HashMap::new();
        let cond = parse_condition("outcome=success && missing=true");
        assert!(!evaluate_condition(&cond, &outcome, &ctx));
    }
}
