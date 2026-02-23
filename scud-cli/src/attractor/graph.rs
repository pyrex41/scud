//! Internal pipeline graph representation using petgraph.

use anyhow::{Context, Result};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::HashMap;
use std::time::Duration;

use super::dot_parser::{AttrValue, DotGraph};

/// The internal pipeline graph built from a parsed DOT file.
#[derive(Debug)]
pub struct PipelineGraph {
    pub name: String,
    pub graph_attrs: GraphAttrs,
    pub graph: DiGraph<PipelineNode, PipelineEdge>,
    pub node_index: HashMap<String, NodeIndex>,
    pub start_node: NodeIndex,
    pub exit_node: NodeIndex,
}

/// Graph-level attributes.
#[derive(Debug, Clone, Default)]
pub struct GraphAttrs {
    pub goal: Option<String>,
    pub fidelity: Option<FidelityMode>,
    pub model_stylesheet: Option<String>,
    pub extra: HashMap<String, String>,
}

/// A node in the pipeline graph.
#[derive(Debug, Clone)]
pub struct PipelineNode {
    pub id: String,
    pub label: String,
    pub shape: String,
    pub handler_type: String,
    pub prompt: String,
    pub max_retries: u32,
    pub goal_gate: bool,
    pub retry_target: Option<String>,
    pub fallback_retry_target: Option<String>,
    pub fidelity: Option<FidelityMode>,
    pub thread_id: Option<String>,
    pub classes: Vec<String>,
    pub timeout: Option<Duration>,
    pub llm_model: Option<String>,
    pub llm_provider: Option<String>,
    pub reasoning_effort: String,
    pub auto_status: bool,
    pub allow_partial: bool,
    pub extra_attrs: HashMap<String, AttrValue>,
}

impl Default for PipelineNode {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            shape: "box".into(),
            handler_type: "codergen".into(),
            prompt: String::new(),
            max_retries: 0,
            goal_gate: false,
            retry_target: None,
            fallback_retry_target: None,
            fidelity: None,
            thread_id: None,
            classes: vec![],
            timeout: None,
            llm_model: None,
            llm_provider: None,
            reasoning_effort: "high".into(),
            auto_status: true,
            allow_partial: false,
            extra_attrs: HashMap::new(),
        }
    }
}

/// An edge in the pipeline graph.
#[derive(Debug, Clone)]
pub struct PipelineEdge {
    pub label: String,
    pub condition: String,
    pub weight: i32,
    pub fidelity: Option<FidelityMode>,
    pub thread_id: Option<String>,
    pub loop_restart: bool,
}

impl Default for PipelineEdge {
    fn default() -> Self {
        Self {
            label: String::new(),
            condition: String::new(),
            weight: 0,
            fidelity: None,
            thread_id: None,
            loop_restart: false,
        }
    }
}

/// Fidelity mode for context passing.
#[derive(Debug, Clone, PartialEq)]
pub enum FidelityMode {
    Full,
    Truncate,
    Compact,
    Summary(SummaryLevel),
}

/// Summary detail level.
#[derive(Debug, Clone, PartialEq)]
pub enum SummaryLevel {
    Low,
    Medium,
    High,
}

impl FidelityMode {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "full" => Some(FidelityMode::Full),
            "truncate" => Some(FidelityMode::Truncate),
            "compact" => Some(FidelityMode::Compact),
            "summary" | "summary-medium" => Some(FidelityMode::Summary(SummaryLevel::Medium)),
            "summary-low" => Some(FidelityMode::Summary(SummaryLevel::Low)),
            "summary-high" => Some(FidelityMode::Summary(SummaryLevel::High)),
            _ => None,
        }
    }
}

/// Shape-to-handler mapping per spec Section 2.8.
fn handler_type_from_shape(shape: &str) -> &str {
    match shape.to_lowercase().as_str() {
        "mdiamond" => "start",
        "msquare" => "exit",
        "box" | "rect" | "rectangle" => "codergen",
        "hexagon" => "wait.human",
        "diamond" => "conditional",
        "component" => "parallel",
        "tripleoctagon" => "parallel.fan_in",
        "parallelogram" => "tool",
        "house" => "stack.manager_loop",
        _ => "codergen", // default
    }
}

impl PipelineGraph {
    /// Build a PipelineGraph from a parsed DotGraph.
    pub fn from_dot(dot: &DotGraph) -> Result<Self> {
        let mut graph = DiGraph::new();
        let mut node_index = HashMap::new();

        // Extract graph-level attrs
        let graph_attrs = GraphAttrs {
            goal: dot
                .graph_attrs
                .get("goal")
                .map(|v| v.as_str()),
            fidelity: dot
                .graph_attrs
                .get("fidelity")
                .and_then(|v| FidelityMode::from_str(&v.as_str())),
            model_stylesheet: dot
                .graph_attrs
                .get("model_stylesheet")
                .map(|v| v.as_str()),
            extra: dot
                .graph_attrs
                .iter()
                .filter(|(k, _)| !["goal", "fidelity", "model_stylesheet"].contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), v.as_str()))
                .collect(),
        };

        // Collect all node IDs (from explicit nodes, edges, and subgraphs)
        let mut all_node_ids: Vec<String> = Vec::new();
        for node in &dot.nodes {
            if !all_node_ids.contains(&node.id) {
                all_node_ids.push(node.id.clone());
            }
        }
        for edge in &dot.edges {
            if !all_node_ids.contains(&edge.from) {
                all_node_ids.push(edge.from.clone());
            }
            if !all_node_ids.contains(&edge.to) {
                all_node_ids.push(edge.to.clone());
            }
        }
        for sg in &dot.subgraphs {
            for node in &sg.nodes {
                if !all_node_ids.contains(&node.id) {
                    all_node_ids.push(node.id.clone());
                }
            }
            for edge in &sg.edges {
                if !all_node_ids.contains(&edge.from) {
                    all_node_ids.push(edge.from.clone());
                }
                if !all_node_ids.contains(&edge.to) {
                    all_node_ids.push(edge.to.clone());
                }
            }
        }

        // Build lookup for node attrs
        let mut node_attrs_map: HashMap<String, HashMap<String, AttrValue>> = HashMap::new();
        for node in &dot.nodes {
            node_attrs_map.insert(node.id.clone(), node.attrs.clone());
        }
        for sg in &dot.subgraphs {
            for node in &sg.nodes {
                node_attrs_map.insert(node.id.clone(), node.attrs.clone());
            }
        }

        // Create petgraph nodes
        for id in &all_node_ids {
            let attrs = node_attrs_map.get(id).cloned().unwrap_or_default();
            let merged_attrs = merge_with_defaults(&attrs, &dot.node_defaults);
            let pipeline_node = build_pipeline_node(id, &merged_attrs);
            let idx = graph.add_node(pipeline_node);
            node_index.insert(id.clone(), idx);
        }

        // Add edges
        let all_edges: Vec<_> = dot
            .edges
            .iter()
            .chain(dot.subgraphs.iter().flat_map(|sg| sg.edges.iter()))
            .collect();

        for edge in all_edges {
            let from_idx = *node_index
                .get(&edge.from)
                .context(format!("Edge source '{}' not found", edge.from))?;
            let to_idx = *node_index
                .get(&edge.to)
                .context(format!("Edge target '{}' not found", edge.to))?;

            let merged = merge_with_defaults(&edge.attrs, &dot.edge_defaults);
            let pipeline_edge = build_pipeline_edge(&merged);
            graph.add_edge(from_idx, to_idx, pipeline_edge);
        }

        // Find start and exit nodes
        let start_node = find_node_by_handler(&graph, &node_index, "start")
            .context("No start node found (need a node with shape=Mdiamond)")?;
        let exit_node = find_node_by_handler(&graph, &node_index, "exit")
            .context("No exit node found (need a node with shape=Msquare)")?;

        Ok(PipelineGraph {
            name: dot.name.clone(),
            graph_attrs,
            graph,
            node_index,
            start_node,
            exit_node,
        })
    }

    /// Get a node by its string ID.
    pub fn node(&self, id: &str) -> Option<&PipelineNode> {
        self.node_index
            .get(id)
            .map(|idx| &self.graph[*idx])
    }

    /// Get all outgoing edges from a node.
    pub fn outgoing_edges(&self, idx: NodeIndex) -> Vec<(NodeIndex, &PipelineEdge)> {
        self.graph
            .edges(idx)
            .map(|e| (e.target(), e.weight()))
            .collect()
    }

    /// Get the node IDs in topological order.
    pub fn topo_order(&self) -> Result<Vec<NodeIndex>> {
        petgraph::algo::toposort(&self.graph, None)
            .map_err(|_| anyhow::anyhow!("Pipeline graph contains a cycle"))
    }
}

fn merge_with_defaults(
    attrs: &HashMap<String, AttrValue>,
    defaults: &HashMap<String, AttrValue>,
) -> HashMap<String, AttrValue> {
    let mut merged = defaults.clone();
    for (k, v) in attrs {
        merged.insert(k.clone(), v.clone());
    }
    merged
}

fn build_pipeline_node(id: &str, attrs: &HashMap<String, AttrValue>) -> PipelineNode {
    let shape = attrs
        .get("shape")
        .map(|v| v.as_str())
        .unwrap_or_else(|| "box".into());

    let explicit_type = attrs.get("type").map(|v| v.as_str());
    let handler_type = explicit_type
        .unwrap_or_else(|| handler_type_from_shape(&shape).into());

    let label = attrs
        .get("label")
        .map(|v| v.as_str())
        .unwrap_or_else(|| id.to_string());

    let classes = attrs
        .get("class")
        .map(|v| {
            v.as_str()
                .split_whitespace()
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    let mut extra_attrs = HashMap::new();
    let known_keys = [
        "shape", "type", "label", "prompt", "max_retries", "goal_gate",
        "retry_target", "fallback_retry_target", "fidelity", "thread_id",
        "class", "timeout", "llm_model", "llm_provider", "reasoning_effort",
        "auto_status", "allow_partial",
    ];
    for (k, v) in attrs {
        if !known_keys.contains(&k.as_str()) {
            extra_attrs.insert(k.clone(), v.clone());
        }
    }

    PipelineNode {
        id: id.to_string(),
        label,
        shape,
        handler_type,
        prompt: attrs.get("prompt").map(|v| v.as_str()).unwrap_or_default(),
        max_retries: attrs
            .get("max_retries")
            .and_then(|v| v.as_int())
            .unwrap_or(0) as u32,
        goal_gate: attrs
            .get("goal_gate")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        retry_target: attrs.get("retry_target").map(|v| v.as_str()),
        fallback_retry_target: attrs.get("fallback_retry_target").map(|v| v.as_str()),
        fidelity: attrs
            .get("fidelity")
            .and_then(|v| FidelityMode::from_str(&v.as_str())),
        thread_id: attrs.get("thread_id").map(|v| v.as_str()),
        classes,
        timeout: attrs.get("timeout").and_then(|v| match v {
            AttrValue::Duration(d) => Some(*d),
            _ => None,
        }),
        llm_model: attrs.get("llm_model").map(|v| v.as_str()),
        llm_provider: attrs.get("llm_provider").map(|v| v.as_str()),
        reasoning_effort: attrs
            .get("reasoning_effort")
            .map(|v| v.as_str())
            .unwrap_or_else(|| "high".into()),
        auto_status: attrs
            .get("auto_status")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        allow_partial: attrs
            .get("allow_partial")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        extra_attrs,
    }
}

fn build_pipeline_edge(attrs: &HashMap<String, AttrValue>) -> PipelineEdge {
    PipelineEdge {
        label: attrs.get("label").map(|v| v.as_str()).unwrap_or_default(),
        condition: attrs.get("condition").map(|v| v.as_str()).unwrap_or_default(),
        weight: attrs
            .get("weight")
            .and_then(|v| v.as_int())
            .unwrap_or(0) as i32,
        fidelity: attrs
            .get("fidelity")
            .and_then(|v| FidelityMode::from_str(&v.as_str())),
        thread_id: attrs.get("thread_id").map(|v| v.as_str()),
        loop_restart: attrs
            .get("loop_restart")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }
}

fn find_node_by_handler(
    graph: &DiGraph<PipelineNode, PipelineEdge>,
    node_index: &HashMap<String, NodeIndex>,
    handler: &str,
) -> Option<NodeIndex> {
    node_index
        .values()
        .copied()
        .find(|idx| graph[*idx].handler_type == handler)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attractor::dot_parser::parse_dot;

    #[test]
    fn test_build_simple_pipeline() {
        let input = r#"
        digraph pipeline {
            graph [goal="Build feature X"]
            start [shape=Mdiamond]
            task_a [shape=box, label="Implement A", prompt="Write the code for A"]
            finish [shape=Msquare]
            start -> task_a -> finish
        }
        "#;
        let dot = parse_dot(input).unwrap();
        let pipeline = PipelineGraph::from_dot(&dot).unwrap();

        assert_eq!(pipeline.name, "pipeline");
        assert_eq!(pipeline.graph_attrs.goal, Some("Build feature X".into()));
        assert_eq!(pipeline.graph.node_count(), 3);
        assert_eq!(pipeline.graph.edge_count(), 2);

        let start = &pipeline.graph[pipeline.start_node];
        assert_eq!(start.handler_type, "start");

        let exit = &pipeline.graph[pipeline.exit_node];
        assert_eq!(exit.handler_type, "exit");

        let task = pipeline.node("task_a").unwrap();
        assert_eq!(task.handler_type, "codergen");
        assert_eq!(task.prompt, "Write the code for A");
    }

    #[test]
    fn test_shape_to_handler_mapping() {
        assert_eq!(handler_type_from_shape("Mdiamond"), "start");
        assert_eq!(handler_type_from_shape("Msquare"), "exit");
        assert_eq!(handler_type_from_shape("box"), "codergen");
        assert_eq!(handler_type_from_shape("hexagon"), "wait.human");
        assert_eq!(handler_type_from_shape("diamond"), "conditional");
        assert_eq!(handler_type_from_shape("component"), "parallel");
        assert_eq!(handler_type_from_shape("tripleoctagon"), "parallel.fan_in");
        assert_eq!(handler_type_from_shape("parallelogram"), "tool");
        assert_eq!(handler_type_from_shape("house"), "stack.manager_loop");
    }

    #[test]
    fn test_outgoing_edges() {
        let input = r#"
        digraph test {
            start [shape=Mdiamond]
            a [shape=box]
            b [shape=box]
            finish [shape=Msquare]
            start -> a [label="go"]
            start -> b [label="alt"]
            a -> finish
            b -> finish
        }
        "#;
        let dot = parse_dot(input).unwrap();
        let pipeline = PipelineGraph::from_dot(&dot).unwrap();

        let edges = pipeline.outgoing_edges(pipeline.start_node);
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn test_node_defaults_applied() {
        let input = r#"
        digraph test {
            node [reasoning_effort="medium"]
            start [shape=Mdiamond]
            a [shape=box]
            finish [shape=Msquare]
            start -> a -> finish
        }
        "#;
        let dot = parse_dot(input).unwrap();
        let pipeline = PipelineGraph::from_dot(&dot).unwrap();
        let a = pipeline.node("a").unwrap();
        assert_eq!(a.reasoning_effort, "medium");
    }
}
