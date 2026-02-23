//! Graph validation with lint rules from spec Section 7.2.

use super::graph::PipelineGraph;

/// Severity of a validation issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// A single validation issue.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub severity: Severity,
    pub rule: String,
    pub message: String,
    pub node_id: Option<String>,
}

/// Validate a pipeline graph and return all issues found.
pub fn validate(graph: &PipelineGraph) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    check_start_node(graph, &mut issues);
    check_terminal_node(graph, &mut issues);
    check_reachability(graph, &mut issues);
    check_edge_targets(graph, &mut issues);
    check_start_no_incoming(graph, &mut issues);
    check_exit_no_outgoing(graph, &mut issues);
    check_condition_syntax(graph, &mut issues);
    check_type_known(graph, &mut issues);
    check_retry_target_exists(graph, &mut issues);
    check_goal_gate_has_retry(graph, &mut issues);
    check_prompt_on_llm_nodes(graph, &mut issues);

    issues
}

/// Check that validation passed (no errors).
pub fn is_valid(issues: &[ValidationIssue]) -> bool {
    !issues.iter().any(|i| i.severity == Severity::Error)
}

fn check_start_node(graph: &PipelineGraph, issues: &mut Vec<ValidationIssue>) {
    let has_start = graph
        .graph
        .node_indices()
        .any(|idx| graph.graph[idx].handler_type == "start");
    if !has_start {
        issues.push(ValidationIssue {
            severity: Severity::Error,
            rule: "start_node".into(),
            message: "Graph must have exactly one start node (shape=Mdiamond)".into(),
            node_id: None,
        });
    }
}

fn check_terminal_node(graph: &PipelineGraph, issues: &mut Vec<ValidationIssue>) {
    let has_exit = graph
        .graph
        .node_indices()
        .any(|idx| graph.graph[idx].handler_type == "exit");
    if !has_exit {
        issues.push(ValidationIssue {
            severity: Severity::Error,
            rule: "terminal_node".into(),
            message: "Graph must have at least one exit node (shape=Msquare)".into(),
            node_id: None,
        });
    }
}

fn check_reachability(graph: &PipelineGraph, issues: &mut Vec<ValidationIssue>) {
    use petgraph::visit::Bfs;

    let mut bfs = Bfs::new(&graph.graph, graph.start_node);
    let mut reachable = std::collections::HashSet::new();
    while let Some(node) = bfs.next(&graph.graph) {
        reachable.insert(node);
    }

    for idx in graph.graph.node_indices() {
        if !reachable.contains(&idx) {
            let node = &graph.graph[idx];
            issues.push(ValidationIssue {
                severity: Severity::Error,
                rule: "reachability".into(),
                message: format!("Node '{}' is not reachable from start", node.id),
                node_id: Some(node.id.clone()),
            });
        }
    }
}

fn check_edge_targets(graph: &PipelineGraph, issues: &mut Vec<ValidationIssue>) {
    // All edge targets exist as nodes (this is guaranteed by petgraph construction,
    // but we check for edges referencing non-existent node IDs in retry_target)
    for idx in graph.graph.node_indices() {
        let node = &graph.graph[idx];
        if let Some(ref target) = node.retry_target {
            if !graph.node_index.contains_key(target) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    rule: "edge_target_exists".into(),
                    message: format!(
                        "Node '{}' has retry_target '{}' which does not exist",
                        node.id, target
                    ),
                    node_id: Some(node.id.clone()),
                });
            }
        }
        if let Some(ref target) = node.fallback_retry_target {
            if !graph.node_index.contains_key(target) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    rule: "edge_target_exists".into(),
                    message: format!(
                        "Node '{}' has fallback_retry_target '{}' which does not exist",
                        node.id, target
                    ),
                    node_id: Some(node.id.clone()),
                });
            }
        }
    }
}

fn check_start_no_incoming(graph: &PipelineGraph, issues: &mut Vec<ValidationIssue>) {
    let incoming = graph
        .graph
        .edges_directed(graph.start_node, petgraph::Direction::Incoming)
        .count();
    if incoming > 0 {
        issues.push(ValidationIssue {
            severity: Severity::Error,
            rule: "start_no_incoming".into(),
            message: "Start node must not have incoming edges".into(),
            node_id: Some(graph.graph[graph.start_node].id.clone()),
        });
    }
}

fn check_exit_no_outgoing(graph: &PipelineGraph, issues: &mut Vec<ValidationIssue>) {
    let outgoing = graph
        .graph
        .edges_directed(graph.exit_node, petgraph::Direction::Outgoing)
        .count();
    if outgoing > 0 {
        issues.push(ValidationIssue {
            severity: Severity::Error,
            rule: "exit_no_outgoing".into(),
            message: "Exit node must not have outgoing edges".into(),
            node_id: Some(graph.graph[graph.exit_node].id.clone()),
        });
    }
}

fn check_condition_syntax(graph: &PipelineGraph, issues: &mut Vec<ValidationIssue>) {
    use petgraph::visit::EdgeRef;
    for edge_ref in graph.graph.edge_references() {
        let edge = edge_ref.weight();
        if !edge.condition.is_empty() {
            // Basic syntax check: must contain = or !=
            let cond = edge.condition.trim();
            if !cond.contains('=') && !cond.contains("!=") {
                let from = &graph.graph[edge_ref.source()].id;
                let to = &graph.graph[edge_ref.target()].id;
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    rule: "condition_syntax".into(),
                    message: format!(
                        "Edge {} -> {} has invalid condition: '{}'",
                        from, to, cond
                    ),
                    node_id: Some(from.clone()),
                });
            }
        }
    }
}

fn check_type_known(graph: &PipelineGraph, issues: &mut Vec<ValidationIssue>) {
    let known_types = [
        "start",
        "exit",
        "codergen",
        "conditional",
        "wait.human",
        "parallel",
        "parallel.fan_in",
        "tool",
        "stack.manager_loop",
    ];

    for idx in graph.graph.node_indices() {
        let node = &graph.graph[idx];
        if !known_types.contains(&node.handler_type.as_str()) {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                rule: "type_known".into(),
                message: format!(
                    "Node '{}' has unknown handler type '{}'",
                    node.id, node.handler_type
                ),
                node_id: Some(node.id.clone()),
            });
        }
    }
}

fn check_retry_target_exists(_graph: &PipelineGraph, _issues: &mut Vec<ValidationIssue>) {
    // Already covered in check_edge_targets, but we can add warning-level checks here
}

fn check_goal_gate_has_retry(graph: &PipelineGraph, issues: &mut Vec<ValidationIssue>) {
    for idx in graph.graph.node_indices() {
        let node = &graph.graph[idx];
        if node.goal_gate && node.retry_target.is_none() {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                rule: "goal_gate_has_retry".into(),
                message: format!(
                    "Node '{}' has goal_gate=true but no retry_target",
                    node.id
                ),
                node_id: Some(node.id.clone()),
            });
        }
    }
}

fn check_prompt_on_llm_nodes(graph: &PipelineGraph, issues: &mut Vec<ValidationIssue>) {
    for idx in graph.graph.node_indices() {
        let node = &graph.graph[idx];
        if node.handler_type == "codergen" && node.prompt.is_empty() {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                rule: "prompt_on_llm_nodes".into(),
                message: format!(
                    "LLM node '{}' has no prompt attribute",
                    node.id
                ),
                node_id: Some(node.id.clone()),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attractor::dot_parser::parse_dot;
    use crate::attractor::graph::PipelineGraph;

    #[test]
    fn test_valid_pipeline() {
        let input = r#"
        digraph test {
            start [shape=Mdiamond]
            task [shape=box, prompt="Do something"]
            finish [shape=Msquare]
            start -> task -> finish
        }
        "#;
        let dot = parse_dot(input).unwrap();
        let graph = PipelineGraph::from_dot(&dot).unwrap();
        let issues = validate(&graph);

        let errors: Vec<_> = issues.iter().filter(|i| i.severity == Severity::Error).collect();
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_unreachable_node() {
        let input = r#"
        digraph test {
            start [shape=Mdiamond]
            task [shape=box, prompt="Do it"]
            orphan [shape=box, prompt="Never reached"]
            finish [shape=Msquare]
            start -> task -> finish
        }
        "#;
        let dot = parse_dot(input).unwrap();
        let graph = PipelineGraph::from_dot(&dot).unwrap();
        let issues = validate(&graph);

        let reachability_errors: Vec<_> = issues
            .iter()
            .filter(|i| i.rule == "reachability")
            .collect();
        assert_eq!(reachability_errors.len(), 1);
        assert!(reachability_errors[0].message.contains("orphan"));
    }

    #[test]
    fn test_missing_prompt_warning() {
        let input = r#"
        digraph test {
            start [shape=Mdiamond]
            task [shape=box]
            finish [shape=Msquare]
            start -> task -> finish
        }
        "#;
        let dot = parse_dot(input).unwrap();
        let graph = PipelineGraph::from_dot(&dot).unwrap();
        let issues = validate(&graph);

        let warnings: Vec<_> = issues
            .iter()
            .filter(|i| i.rule == "prompt_on_llm_nodes")
            .collect();
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn test_goal_gate_without_retry_warning() {
        let input = r#"
        digraph test {
            start [shape=Mdiamond]
            task [shape=box, prompt="Do it"]
            gate [shape=Msquare, goal_gate=true]
            start -> task -> gate
        }
        "#;
        let dot = parse_dot(input).unwrap();
        let graph = PipelineGraph::from_dot(&dot).unwrap();
        let issues = validate(&graph);

        let warnings: Vec<_> = issues
            .iter()
            .filter(|i| i.rule == "goal_gate_has_retry")
            .collect();
        assert_eq!(warnings.len(), 1);
    }
}
