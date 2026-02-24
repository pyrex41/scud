//! DOT subset parser for Attractor pipeline graphs.
//!
//! Parses the Attractor DOT dialect: one digraph per file, directed edges only,
//! node/edge attributes, chained edges, subgraph scoping, default blocks.

use anyhow::{bail, Context, Result};
use std::collections::HashMap;

/// A parsed DOT graph.
#[derive(Debug, Clone)]
pub struct DotGraph {
    pub name: String,
    pub graph_attrs: HashMap<String, AttrValue>,
    pub nodes: Vec<DotNode>,
    pub edges: Vec<DotEdge>,
    pub subgraphs: Vec<DotSubgraph>,
    pub node_defaults: HashMap<String, AttrValue>,
    pub edge_defaults: HashMap<String, AttrValue>,
}

/// A node in the DOT graph.
#[derive(Debug, Clone)]
pub struct DotNode {
    pub id: String,
    pub attrs: HashMap<String, AttrValue>,
}

/// An edge in the DOT graph.
#[derive(Debug, Clone)]
pub struct DotEdge {
    pub from: String,
    pub to: String,
    pub attrs: HashMap<String, AttrValue>,
}

/// A subgraph (cluster) in the DOT graph.
#[derive(Debug, Clone)]
pub struct DotSubgraph {
    pub name: Option<String>,
    pub attrs: HashMap<String, AttrValue>,
    pub nodes: Vec<DotNode>,
    pub edges: Vec<DotEdge>,
}

/// Typed attribute value.
#[derive(Debug, Clone, PartialEq)]
pub enum AttrValue {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Duration(std::time::Duration),
}

impl AttrValue {
    /// Get the value as a string, regardless of type.
    pub fn as_str(&self) -> String {
        match self {
            AttrValue::Str(s) => s.clone(),
            AttrValue::Int(i) => i.to_string(),
            AttrValue::Float(f) => f.to_string(),
            AttrValue::Bool(b) => b.to_string(),
            AttrValue::Duration(d) => format!("{}s", d.as_secs()),
        }
    }

    /// Get the value as a string reference if it is a string.
    pub fn str_ref(&self) -> Option<&str> {
        match self {
            AttrValue::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Get the value as an integer if possible.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            AttrValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Get the value as a bool if possible.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            AttrValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

/// Parse a DOT file string into a DotGraph.
pub fn parse_dot(input: &str) -> Result<DotGraph> {
    let mut parser = DotParser::new(input);
    parser.parse()
}

struct DotParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> DotParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn parse(&mut self) -> Result<DotGraph> {
        self.skip_ws();

        // Expect "digraph"
        self.expect_keyword("digraph")
            .context("Expected 'digraph' keyword")?;
        self.skip_ws();

        // Graph name (optional)
        let name = if self.peek_char() != Some('{') {
            self.read_identifier()?
        } else {
            String::new()
        };
        self.skip_ws();

        self.expect_char('{')?;

        let mut graph = DotGraph {
            name,
            graph_attrs: HashMap::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
            node_defaults: HashMap::new(),
            edge_defaults: HashMap::new(),
        };

        self.parse_body(&mut graph)?;

        self.skip_ws();
        self.expect_char('}')?;

        Ok(graph)
    }

    fn parse_body(&mut self, graph: &mut DotGraph) -> Result<()> {
        loop {
            self.skip_ws();
            if self.peek_char() == Some('}') || self.is_eof() {
                break;
            }

            // Skip comments
            if self.peek_str("//") {
                self.skip_line();
                continue;
            }
            if self.peek_str("/*") {
                self.skip_block_comment();
                continue;
            }

            // Check for keyword statements
            if self.peek_keyword("node") {
                self.advance(4);
                self.skip_ws();
                if self.peek_char() == Some('[') {
                    let attrs = self.parse_attr_list()?;
                    for (k, v) in attrs {
                        graph.node_defaults.insert(k, v);
                    }
                }
                self.skip_optional_semicolon();
                continue;
            }

            if self.peek_keyword("edge") {
                self.advance(4);
                self.skip_ws();
                if self.peek_char() == Some('[') {
                    let attrs = self.parse_attr_list()?;
                    for (k, v) in attrs {
                        graph.edge_defaults.insert(k, v);
                    }
                }
                self.skip_optional_semicolon();
                continue;
            }

            if self.peek_keyword("graph") {
                self.advance(5);
                self.skip_ws();
                if self.peek_char() == Some('[') {
                    let attrs = self.parse_attr_list()?;
                    for (k, v) in attrs {
                        graph.graph_attrs.insert(k, v);
                    }
                }
                self.skip_optional_semicolon();
                continue;
            }

            // Check for subgraph
            if self.peek_keyword("subgraph") {
                let sg = self.parse_subgraph()?;
                graph.subgraphs.push(sg);
                self.skip_optional_semicolon();
                continue;
            }

            // Must be a node or edge statement
            let id = self.read_identifier_or_quoted()?;
            self.skip_ws();

            if self.peek_str("->") {
                // Edge chain: a -> b -> c [attrs]
                let mut chain = vec![id];
                while self.peek_str("->") {
                    self.advance(2);
                    self.skip_ws();
                    chain.push(self.read_identifier_or_quoted()?);
                    self.skip_ws();
                }

                let attrs = if self.peek_char() == Some('[') {
                    self.parse_attr_list()?
                } else {
                    HashMap::new()
                };

                // Expand chain into individual edges
                for window in chain.windows(2) {
                    graph.edges.push(DotEdge {
                        from: window[0].clone(),
                        to: window[1].clone(),
                        attrs: attrs.clone(),
                    });
                }
            } else {
                // Node statement with optional attrs
                let attrs = if self.peek_char() == Some('[') {
                    self.parse_attr_list()?
                } else {
                    HashMap::new()
                };

                graph.nodes.push(DotNode {
                    id,
                    attrs,
                });
            }

            self.skip_optional_semicolon();
        }

        Ok(())
    }

    fn parse_subgraph(&mut self) -> Result<DotSubgraph> {
        self.expect_keyword("subgraph")?;
        self.skip_ws();

        let name = if self.peek_char() != Some('{') {
            Some(self.read_identifier_or_quoted()?)
        } else {
            None
        };
        self.skip_ws();
        self.expect_char('{')?;

        let mut sg = DotSubgraph {
            name,
            attrs: HashMap::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
        };

        // Parse subgraph body (simplified — no nested subgraphs)
        loop {
            self.skip_ws();
            if self.peek_char() == Some('}') || self.is_eof() {
                break;
            }
            if self.peek_str("//") {
                self.skip_line();
                continue;
            }
            if self.peek_str("/*") {
                self.skip_block_comment();
                continue;
            }

            // Check for graph/node/edge defaults
            if self.peek_keyword("graph") {
                self.advance(5);
                self.skip_ws();
                if self.peek_char() == Some('[') {
                    let attrs = self.parse_attr_list()?;
                    sg.attrs.extend(attrs);
                }
                self.skip_optional_semicolon();
                continue;
            }

            let id = self.read_identifier_or_quoted()?;
            self.skip_ws();

            if self.peek_str("->") {
                let mut chain = vec![id];
                while self.peek_str("->") {
                    self.advance(2);
                    self.skip_ws();
                    chain.push(self.read_identifier_or_quoted()?);
                    self.skip_ws();
                }
                let attrs = if self.peek_char() == Some('[') {
                    self.parse_attr_list()?
                } else {
                    HashMap::new()
                };
                for window in chain.windows(2) {
                    sg.edges.push(DotEdge {
                        from: window[0].clone(),
                        to: window[1].clone(),
                        attrs: attrs.clone(),
                    });
                }
            } else {
                let attrs = if self.peek_char() == Some('[') {
                    self.parse_attr_list()?
                } else {
                    HashMap::new()
                };
                sg.nodes.push(DotNode { id, attrs });
            }

            self.skip_optional_semicolon();
        }

        self.expect_char('}')?;
        Ok(sg)
    }

    fn parse_attr_list(&mut self) -> Result<HashMap<String, AttrValue>> {
        self.expect_char('[')?;
        let mut attrs = HashMap::new();

        loop {
            self.skip_ws();
            if self.peek_char() == Some(']') {
                self.advance(1);
                break;
            }

            let key = self.read_identifier()?;
            self.skip_ws();
            self.expect_char('=')?;
            self.skip_ws();
            let value = self.read_attr_value()?;
            attrs.insert(key, value);

            self.skip_ws();
            // Optional comma or semicolon separator
            if self.peek_char() == Some(',') || self.peek_char() == Some(';') {
                self.advance(1);
            }
        }

        Ok(attrs)
    }

    fn read_attr_value(&mut self) -> Result<AttrValue> {
        let ch = self.peek_char().context("Unexpected EOF in attribute value")?;

        if ch == '"' {
            let s = self.read_quoted_string()?;
            // Try to parse as duration (e.g., "30s", "5m")
            if let Some(d) = parse_duration_str(&s) {
                return Ok(AttrValue::Duration(d));
            }
            Ok(AttrValue::Str(s))
        } else if ch == '-' || ch.is_ascii_digit() {
            let num_str = self.read_number_str();
            if num_str.contains('.') {
                Ok(AttrValue::Float(
                    num_str.parse().context("Invalid float")?,
                ))
            } else {
                Ok(AttrValue::Int(num_str.parse().context("Invalid integer")?))
            }
        } else {
            // Bare word — could be bool or string
            let word = self.read_identifier()?;
            match word.to_lowercase().as_str() {
                "true" | "yes" => Ok(AttrValue::Bool(true)),
                "false" | "no" => Ok(AttrValue::Bool(false)),
                _ => Ok(AttrValue::Str(word)),
            }
        }
    }

    fn read_quoted_string(&mut self) -> Result<String> {
        self.expect_char('"')?;
        let mut s = String::new();
        loop {
            match self.next_char() {
                Some('\\') => match self.next_char() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('"') => s.push('"'),
                    Some('\\') => s.push('\\'),
                    Some(c) => {
                        s.push('\\');
                        s.push(c);
                    }
                    None => bail!("Unterminated escape in string"),
                },
                Some('"') => break,
                Some(c) => s.push(c),
                None => bail!("Unterminated string"),
            }
        }
        Ok(s)
    }

    fn read_identifier(&mut self) -> Result<String> {
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_alphanumeric() || c == '_' || c == '.' || c == '-' {
                self.advance(1);
            } else {
                break;
            }
        }
        if self.pos == start {
            bail!(
                "Expected identifier at position {}, got {:?}",
                self.pos,
                self.peek_char()
            );
        }
        Ok(self.input[start..self.pos].to_string())
    }

    fn read_identifier_or_quoted(&mut self) -> Result<String> {
        if self.peek_char() == Some('"') {
            self.read_quoted_string()
        } else {
            self.read_identifier()
        }
    }

    fn read_number_str(&mut self) -> String {
        let start = self.pos;
        if self.peek_char() == Some('-') {
            self.advance(1);
        }
        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() || c == '.' {
                self.advance(1);
            } else {
                break;
            }
        }
        self.input[start..self.pos].to_string()
    }

    // --- Utility methods ---

    fn skip_ws(&mut self) {
        loop {
            match self.peek_char() {
                Some(c) if c.is_whitespace() => {
                    self.advance(1);
                }
                Some('/') if self.peek_str("//") => {
                    self.skip_line();
                }
                Some('/') if self.peek_str("/*") => {
                    self.skip_block_comment();
                }
                _ => break,
            }
        }
    }

    fn skip_line(&mut self) {
        while let Some(c) = self.next_char() {
            if c == '\n' {
                break;
            }
        }
    }

    fn skip_block_comment(&mut self) {
        self.advance(2); // skip /*
        while !self.is_eof() {
            if self.peek_str("*/") {
                self.advance(2);
                return;
            }
            self.advance(1);
        }
    }

    fn skip_optional_semicolon(&mut self) {
        self.skip_ws();
        if self.peek_char() == Some(';') {
            self.advance(1);
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn next_char(&mut self) -> Option<char> {
        let c = self.input[self.pos..].chars().next()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn advance(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.input.len());
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn peek_str(&self, s: &str) -> bool {
        self.input[self.pos..].starts_with(s)
    }

    fn peek_keyword(&self, kw: &str) -> bool {
        if !self.input[self.pos..].starts_with(kw) {
            return false;
        }
        // Must be followed by non-identifier char
        let after = self.pos + kw.len();
        if after >= self.input.len() {
            return true;
        }
        let next = self.input[after..].chars().next().unwrap();
        !next.is_alphanumeric() && next != '_'
    }

    fn expect_keyword(&mut self, kw: &str) -> Result<()> {
        if !self.peek_keyword(kw) {
            bail!(
                "Expected '{}' at position {}, got '{}'",
                kw,
                self.pos,
                &self.input[self.pos..self.pos + 10.min(self.input.len() - self.pos)]
            );
        }
        self.advance(kw.len());
        Ok(())
    }

    fn expect_char(&mut self, expected: char) -> Result<()> {
        match self.next_char() {
            Some(c) if c == expected => Ok(()),
            Some(c) => bail!("Expected '{}', got '{}' at position {}", expected, c, self.pos - 1),
            None => bail!("Expected '{}', got EOF", expected),
        }
    }
}

/// Parse a duration string like "30s", "5m", "1h".
fn parse_duration_str(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    if s.ends_with('s') {
        let n: u64 = s[..s.len() - 1].parse().ok()?;
        Some(std::time::Duration::from_secs(n))
    } else if s.ends_with('m') {
        let n: u64 = s[..s.len() - 1].parse().ok()?;
        Some(std::time::Duration::from_secs(n * 60))
    } else if s.ends_with('h') {
        let n: u64 = s[..s.len() - 1].parse().ok()?;
        Some(std::time::Duration::from_secs(n * 3600))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_digraph() {
        let input = r#"
        digraph pipeline {
            start [shape=Mdiamond]
            task_a [shape=box, label="Do task A", prompt="Write code"]
            finish [shape=Msquare]

            start -> task_a -> finish
        }
        "#;
        let graph = parse_dot(input).unwrap();
        assert_eq!(graph.name, "pipeline");
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.edges[0].from, "start");
        assert_eq!(graph.edges[0].to, "task_a");
    }

    #[test]
    fn test_parse_graph_attrs() {
        let input = r#"
        digraph test {
            graph [goal="Build a feature", fidelity="full"]
            a -> b
        }
        "#;
        let graph = parse_dot(input).unwrap();
        assert_eq!(
            graph.graph_attrs.get("goal"),
            Some(&AttrValue::Str("Build a feature".into()))
        );
    }

    #[test]
    fn test_parse_node_defaults() {
        let input = r#"
        digraph test {
            node [shape=box, reasoning_effort="high"]
            a
            b
            a -> b
        }
        "#;
        let graph = parse_dot(input).unwrap();
        assert_eq!(
            graph.node_defaults.get("shape"),
            Some(&AttrValue::Str("box".into()))
        );
    }

    #[test]
    fn test_parse_edge_with_attrs() {
        let input = r#"
        digraph test {
            a -> b [label="success", condition="outcome=success", weight=10]
        }
        "#;
        let graph = parse_dot(input).unwrap();
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(
            graph.edges[0].attrs.get("label"),
            Some(&AttrValue::Str("success".into()))
        );
        assert_eq!(
            graph.edges[0].attrs.get("weight"),
            Some(&AttrValue::Int(10))
        );
    }

    #[test]
    fn test_parse_chained_edges() {
        let input = r#"
        digraph test {
            a -> b -> c -> d [label="chain"]
        }
        "#;
        let graph = parse_dot(input).unwrap();
        assert_eq!(graph.edges.len(), 3);
        assert_eq!(graph.edges[0].from, "a");
        assert_eq!(graph.edges[0].to, "b");
        assert_eq!(graph.edges[2].from, "c");
        assert_eq!(graph.edges[2].to, "d");
    }

    #[test]
    fn test_parse_bool_and_int_attrs() {
        let input = r#"
        digraph test {
            a [goal_gate=true, max_retries=3, auto_status=false]
        }
        "#;
        let graph = parse_dot(input).unwrap();
        let node = &graph.nodes[0];
        assert_eq!(node.attrs.get("goal_gate"), Some(&AttrValue::Bool(true)));
        assert_eq!(node.attrs.get("max_retries"), Some(&AttrValue::Int(3)));
        assert_eq!(node.attrs.get("auto_status"), Some(&AttrValue::Bool(false)));
    }

    #[test]
    fn test_parse_duration_attr() {
        let input = r#"
        digraph test {
            a [timeout="30s"]
        }
        "#;
        let graph = parse_dot(input).unwrap();
        assert_eq!(
            graph.nodes[0].attrs.get("timeout"),
            Some(&AttrValue::Duration(std::time::Duration::from_secs(30)))
        );
    }

    #[test]
    fn test_parse_comments() {
        let input = r#"
        // This is a comment
        digraph test {
            /* block comment */
            a -> b // inline comment
        }
        "#;
        let graph = parse_dot(input).unwrap();
        assert_eq!(graph.edges.len(), 1);
    }

    #[test]
    fn test_parse_subgraph() {
        let input = r#"
        digraph test {
            subgraph cluster_parallel {
                graph [label="Parallel branch"]
                p1 [shape=box]
                p2 [shape=box]
            }
            start -> p1
        }
        "#;
        let graph = parse_dot(input).unwrap();
        assert_eq!(graph.subgraphs.len(), 1);
        assert_eq!(graph.subgraphs[0].nodes.len(), 2);
    }

    #[test]
    fn test_parse_quoted_identifiers() {
        let input = r#"
        digraph test {
            "node with spaces" [label="A node"]
            "node with spaces" -> b
        }
        "#;
        let graph = parse_dot(input).unwrap();
        assert_eq!(graph.nodes[0].id, "node with spaces");
    }

    #[test]
    fn test_parse_empty_graph() {
        let input = "digraph empty {}";
        let graph = parse_dot(input).unwrap();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }
}
