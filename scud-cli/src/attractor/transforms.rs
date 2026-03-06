//! Pre-execution graph transforms.
//!
//! Applied in order after parsing, before validation:
//! 1. Variable expansion: Replace `$goal` in node prompts
//! 2. Stylesheet application: Apply model_stylesheet CSS-like rules

use super::graph::PipelineGraph;
use super::stylesheet::{apply_stylesheet, parse_stylesheet};

/// Apply all transforms to a pipeline graph.
pub fn apply_transforms(graph: &mut PipelineGraph) {
    expand_goal_variables(graph);
    apply_stylesheet_transform(graph);
}

/// Replace `$goal` in all node prompts with the graph-level goal attribute.
fn expand_goal_variables(graph: &mut PipelineGraph) {
    let goal = match &graph.graph_attrs.goal {
        Some(g) => g.clone(),
        None => return,
    };

    for node_idx in graph.graph.node_indices() {
        let node = &mut graph.graph[node_idx];
        if node.prompt.contains("$goal") {
            node.prompt = node.prompt.replace("$goal", &goal);
        }
    }
}

/// Apply model_stylesheet rules to resolve llm_model, llm_provider, reasoning_effort.
fn apply_stylesheet_transform(graph: &mut PipelineGraph) {
    let stylesheet_src = match &graph.graph_attrs.model_stylesheet {
        Some(s) => s.clone(),
        None => return,
    };

    let rules = match parse_stylesheet(&stylesheet_src) {
        Ok(r) => r,
        Err(_) => return, // Silently skip invalid stylesheets (validator will catch it)
    };

    apply_stylesheet(graph, &rules);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attractor::dot_parser::parse_dot;
    use crate::attractor::graph::PipelineGraph;

    #[test]
    fn test_goal_expansion() {
        let input = r#"
        digraph test {
            graph [goal="Build a REST API"]
            start [shape=Mdiamond]
            task [shape=box, prompt="Your goal: $goal. Do it."]
            finish [shape=Msquare]
            start -> task -> finish
        }
        "#;
        let dot = parse_dot(input).unwrap();
        let mut graph = PipelineGraph::from_dot(&dot).unwrap();
        apply_transforms(&mut graph);

        let task = graph.node("task").unwrap();
        assert_eq!(task.prompt, "Your goal: Build a REST API. Do it.");
    }
}
