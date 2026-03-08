//! CSS-like model/provider stylesheet parser and applicator.
//!
//! Selectors:
//! - `*` (specificity 0) — matches all nodes
//! - `.class` (specificity 1) — matches nodes with that class
//! - `#node_id` (specificity 2) — matches a specific node
//!
//! Later rules of equal specificity win.
//! Explicit node attributes always override stylesheet values.

use anyhow::{bail, Result};
use std::collections::HashMap;

use super::graph::PipelineGraph;

/// A single stylesheet rule.
#[derive(Debug, Clone)]
pub struct StyleRule {
    pub selector: Selector,
    pub properties: HashMap<String, String>,
}

/// A CSS-like selector.
#[derive(Debug, Clone)]
pub enum Selector {
    /// `*` — matches all nodes.
    Universal,
    /// `.classname` — matches nodes with that class.
    Class(String),
    /// `#node_id` — matches a specific node by ID.
    Id(String),
}

impl Selector {
    /// Specificity value (higher = more specific).
    pub fn specificity(&self) -> u8 {
        match self {
            Selector::Universal => 0,
            Selector::Class(_) => 1,
            Selector::Id(_) => 2,
        }
    }

    /// Check if this selector matches a node.
    pub fn matches(&self, node_id: &str, node_classes: &[String]) -> bool {
        match self {
            Selector::Universal => true,
            Selector::Class(class) => node_classes.iter().any(|c| c == class),
            Selector::Id(id) => node_id == id,
        }
    }
}

/// Parse a stylesheet string into rules.
///
/// Format:
/// ```text
/// * { model: "claude-3-opus"; reasoning_effort: "high" }
/// .fast { model: "claude-3-haiku"; reasoning_effort: "low" }
/// #critical_node { model: "claude-3-opus"; provider: "anthropic" }
/// ```
pub fn parse_stylesheet(input: &str) -> Result<Vec<StyleRule>> {
    let mut rules = Vec::new();
    let mut chars = input.chars().peekable();

    loop {
        // Skip whitespace
        while chars.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
            chars.next();
        }

        if chars.peek().is_none() {
            break;
        }

        // Parse selector
        let selector = parse_selector(&mut chars)?;

        // Skip whitespace
        while chars.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
            chars.next();
        }

        // Expect {
        match chars.next() {
            Some('{') => {}
            _ => bail!("Expected '{{' after selector"),
        }

        // Parse properties
        let mut properties = HashMap::new();
        loop {
            // Skip whitespace
            while chars.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
                chars.next();
            }

            if chars.peek() == Some(&'}') {
                chars.next();
                break;
            }

            if chars.peek().is_none() {
                bail!("Unterminated rule block");
            }

            // Read property name
            let mut name = String::new();
            while let Some(&c) = chars.peek() {
                if c == ':' || c.is_whitespace() {
                    break;
                }
                name.push(c);
                chars.next();
            }

            // Skip whitespace and colon
            while chars.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
                chars.next();
            }
            if chars.peek() == Some(&':') {
                chars.next();
            }
            while chars.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
                chars.next();
            }

            // Read value (quoted or bare)
            let value = if chars.peek() == Some(&'"') {
                chars.next(); // skip opening quote
                let mut v = String::new();
                while let Some(c) = chars.next() {
                    if c == '"' {
                        break;
                    }
                    v.push(c);
                }
                v
            } else {
                let mut v = String::new();
                while let Some(&c) = chars.peek() {
                    if c == ';' || c == '}' || c.is_whitespace() {
                        break;
                    }
                    v.push(c);
                    chars.next();
                }
                v
            };

            if !name.is_empty() {
                properties.insert(name, value);
            }

            // Skip optional semicolon
            while chars.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
                chars.next();
            }
            if chars.peek() == Some(&';') {
                chars.next();
            }
        }

        rules.push(StyleRule {
            selector,
            properties,
        });
    }

    Ok(rules)
}

fn parse_selector(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<Selector> {
    match chars.peek() {
        Some('*') => {
            chars.next();
            Ok(Selector::Universal)
        }
        Some('.') => {
            chars.next();
            let mut name = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    name.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            Ok(Selector::Class(name))
        }
        Some('#') => {
            chars.next();
            let mut name = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    name.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            Ok(Selector::Id(name))
        }
        Some(c) => bail!("Invalid selector start: '{}'", c),
        None => bail!("Expected selector, got EOF"),
    }
}

/// Apply stylesheet rules to a pipeline graph.
///
/// Rules are applied in order of specificity (lowest first).
/// Explicit node attributes always override stylesheet values.
pub fn apply_stylesheet(graph: &mut PipelineGraph, rules: &[StyleRule]) {
    // Sort rules by specificity (stable sort preserves declaration order)
    let mut sorted_rules: Vec<_> = rules.iter().collect();
    sorted_rules.sort_by_key(|r| r.selector.specificity());

    for node_idx in graph.graph.node_indices() {
        let (node_id, node_classes, has_model, has_provider, has_effort) = {
            let node = &graph.graph[node_idx];
            (
                node.id.clone(),
                node.classes.clone(),
                node.llm_model.is_some(),
                node.llm_provider.is_some(),
                node.reasoning_effort != "high", // "high" is default
            )
        };

        for rule in &sorted_rules {
            if rule.selector.matches(&node_id, &node_classes) {
                let node = &mut graph.graph[node_idx];

                // Only apply if the node doesn't have an explicit value
                if let Some(model) = rule.properties.get("model") {
                    if !has_model {
                        node.llm_model = Some(model.clone());
                    }
                }
                if let Some(provider) = rule.properties.get("provider") {
                    if !has_provider {
                        node.llm_provider = Some(provider.clone());
                    }
                }
                if let Some(effort) = rule.properties.get("reasoning_effort") {
                    if !has_effort {
                        node.reasoning_effort = effort.clone();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_stylesheet() {
        let input = r#"
            * { model: "claude-3-haiku"; reasoning_effort: "medium" }
            .critical { model: "claude-3-opus" }
            #special_node { provider: "anthropic" }
        "#;
        let rules = parse_stylesheet(input).unwrap();
        assert_eq!(rules.len(), 3);
        assert!(matches!(rules[0].selector, Selector::Universal));
        assert!(matches!(rules[1].selector, Selector::Class(ref c) if c == "critical"));
        assert!(matches!(rules[2].selector, Selector::Id(ref id) if id == "special_node"));
    }

    #[test]
    fn test_selector_specificity() {
        assert_eq!(Selector::Universal.specificity(), 0);
        assert_eq!(Selector::Class("x".into()).specificity(), 1);
        assert_eq!(Selector::Id("x".into()).specificity(), 2);
    }

    #[test]
    fn test_selector_matches() {
        assert!(Selector::Universal.matches("any", &[]));
        assert!(Selector::Class("fast".into()).matches("x", &["fast".into()]));
        assert!(!Selector::Class("fast".into()).matches("x", &["slow".into()]));
        assert!(Selector::Id("x".into()).matches("x", &[]));
        assert!(!Selector::Id("x".into()).matches("y", &[]));
    }

    #[test]
    fn test_apply_stylesheet() {
        use crate::attractor::dot_parser::parse_dot;
        use crate::attractor::graph::PipelineGraph;

        let input = r#"
        digraph test {
            graph [model_stylesheet="* { model: \"haiku\" }"]
            start [shape=Mdiamond]
            a [shape=box, class="fast"]
            b [shape=box, llm_model="opus"]
            finish [shape=Msquare]
            start -> a -> b -> finish
        }
        "#;
        let dot = parse_dot(input).unwrap();
        let mut graph = PipelineGraph::from_dot(&dot).unwrap();

        let rules = parse_stylesheet("* { model: \"haiku\" }").unwrap();
        apply_stylesheet(&mut graph, &rules);

        // Node 'a' should get model from stylesheet
        let a = graph.node("a").unwrap();
        assert_eq!(a.llm_model, Some("haiku".into()));

        // Node 'b' already has explicit model, should keep it
        let b = graph.node("b").unwrap();
        assert_eq!(b.llm_model, Some("opus".into()));
    }
}
