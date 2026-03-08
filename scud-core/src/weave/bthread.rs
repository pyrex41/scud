//! B-Thread definitions and rule types.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::event::{EventKind, EventPattern};
use super::matcher::GlobPattern;

/// A behavioral thread — a named, prioritized set of coordination rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BThread {
    /// Thread ID, e.g. "w:1"
    pub id: String,
    pub name: String,
    /// Lower number = higher priority.
    pub priority: u32,
    pub enabled: bool,
    pub rules: Vec<BThreadRule>,
}

/// Rule types for b-thread coordination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BThreadRule {
    /// Only one agent can hold a resource at a time.
    Mutex {
        scope: EventPattern,
        /// Template key, e.g. "file:{target}" or "schema-global"
        key: String,
        ttl_secs: Option<u64>,
    },
    /// Action X requires prior event Y (with optional reset).
    Require {
        trigger: EventPattern,
        prerequisite: EventPattern,
        reset: Option<EventPattern>,
    },
    /// Block event Y after event X until event Z occurs.
    BlockUntil {
        trigger: EventPattern,
        block: Vec<EventPattern>,
        until: EventPattern,
        #[serde(default)]
        escalate: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        escalation_message: Option<String>,
    },
    /// Unconditionally block matching events.
    BlockAlways {
        scope: EventPattern,
    },
    /// Max N events in a time window.
    RateLimit {
        scope: EventPattern,
        max: u32,
        window_secs: u64,
    },
    /// Kill operations exceeding a time budget.
    Timeout {
        scope: EventPattern,
        max_duration_secs: u64,
        action: TimeoutAction,
    },
    /// Deterministic work sharding across agents.
    Partition {
        scope: EventPattern,
        strategy: PartitionStrategy,
        agent_count: u32,
    },
}

impl BThreadRule {
    /// Parse a rule from its type name and spec string.
    ///
    /// The spec is space-separated `key=value` pairs.
    /// Example: `kind=FileWrite key=file:{target}` with rule_type="Mutex"
    pub fn parse(rule_type: &str, spec: &str) -> Result<BThreadRule> {
        let params = parse_key_values(spec);

        match rule_type {
            "Mutex" => {
                let scope = pattern_from_params(&params)?;
                let key = params
                    .get("key")
                    .ok_or_else(|| anyhow!("Mutex rule requires 'key' parameter"))?
                    .clone();
                let ttl_secs = params.get("ttl").map(|v| v.parse::<u64>()).transpose()?;
                Ok(BThreadRule::Mutex {
                    scope,
                    key,
                    ttl_secs,
                })
            }
            "Require" => {
                let trigger_kind = params
                    .get("trigger")
                    .ok_or_else(|| anyhow!("Require rule requires 'trigger' parameter"))?;
                let prereq_kind = params
                    .get("prereq")
                    .ok_or_else(|| anyhow!("Require rule requires 'prereq' parameter"))?;
                let reset = params
                    .get("reset")
                    .map(|r| EventPattern::kind(EventKind::parse(r)));
                Ok(BThreadRule::Require {
                    trigger: EventPattern::kind(EventKind::parse(trigger_kind)),
                    prerequisite: EventPattern::kind(EventKind::parse(prereq_kind)),
                    reset,
                })
            }
            "BlockUntil" => {
                let trigger_kind = params
                    .get("trigger")
                    .ok_or_else(|| anyhow!("BlockUntil rule requires 'trigger' parameter"))?;
                let block_kinds = params
                    .get("block")
                    .ok_or_else(|| anyhow!("BlockUntil rule requires 'block' parameter"))?;
                let until_kind = params
                    .get("until")
                    .ok_or_else(|| anyhow!("BlockUntil rule requires 'until' parameter"))?;
                let escalate = params
                    .get("escalate")
                    .map(|v| v == "true" || v == "Y")
                    .unwrap_or(false);
                let escalation_message = params.get("message").cloned();
                let block = block_kinds
                    .split(',')
                    .map(|k| EventPattern::kind(EventKind::parse(k.trim())))
                    .collect();
                Ok(BThreadRule::BlockUntil {
                    trigger: EventPattern::kind(EventKind::parse(trigger_kind)),
                    block,
                    until: EventPattern::kind(EventKind::parse(until_kind)),
                    escalate,
                    escalation_message,
                })
            }
            "BlockAlways" => {
                let scope = pattern_from_params(&params)?;
                Ok(BThreadRule::BlockAlways { scope })
            }
            "RateLimit" => {
                let scope = pattern_from_params(&params)?;
                let max = params
                    .get("max")
                    .ok_or_else(|| anyhow!("RateLimit rule requires 'max' parameter"))?
                    .parse::<u32>()?;
                let window_secs = params
                    .get("window")
                    .ok_or_else(|| anyhow!("RateLimit rule requires 'window' parameter"))?
                    .parse::<u64>()?;
                Ok(BThreadRule::RateLimit {
                    scope,
                    max,
                    window_secs,
                })
            }
            "Timeout" => {
                let scope = pattern_from_params(&params)?;
                let max_duration_secs = params
                    .get("max_secs")
                    .ok_or_else(|| anyhow!("Timeout rule requires 'max_secs' parameter"))?
                    .parse::<u64>()?;
                let action = match params.get("action").map(|s| s.as_str()) {
                    Some("Warn" | "warn") => TimeoutAction::Warn,
                    _ => TimeoutAction::Kill,
                };
                Ok(BThreadRule::Timeout {
                    scope,
                    max_duration_secs,
                    action,
                })
            }
            "Partition" => {
                let scope = pattern_from_params(&params)?;
                let strategy = match params.get("strategy").map(|s| s.as_str()) {
                    Some("round-robin" | "RoundRobin") => PartitionStrategy::RoundRobin,
                    Some("directory" | "Directory") => PartitionStrategy::Directory,
                    _ => PartitionStrategy::Hash,
                };
                let agent_count = params
                    .get("count")
                    .or_else(|| params.get("agent_count"))
                    .ok_or_else(|| {
                        anyhow!("Partition rule requires 'count' or 'agent_count' parameter")
                    })?
                    .parse::<u32>()?;
                Ok(BThreadRule::Partition {
                    scope,
                    strategy,
                    agent_count,
                })
            }
            other => Err(anyhow!("Unknown rule type: {}", other)),
        }
    }
}

/// Parse space-separated `key=value` pairs from a spec string.
fn parse_key_values(spec: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for token in spec.split_whitespace() {
        if let Some((k, v)) = token.split_once('=') {
            result.insert(k.to_string(), v.to_string());
        }
    }
    result
}

/// Build an EventPattern from parsed parameters (using `kind`, `target`, `agent` keys).
fn pattern_from_params(params: &HashMap<String, String>) -> Result<EventPattern> {
    let kind = params.get("kind").map(|k| EventKind::parse(k));
    let agent = params.get("agent").cloned();
    let target = params.get("target").map(|t| GlobPattern::new(t));
    let negate_agent = params
        .get("negate_agent")
        .map(|v| v == "true" || v == "Y")
        .unwrap_or(false);
    let target_not = params
        .get("target_not")
        .map(|v| {
            v.split(',')
                .map(|p| GlobPattern::new(p.trim()))
                .collect()
        })
        .unwrap_or_default();
    Ok(EventPattern {
        kind,
        agent,
        target,
        task_id: None,
        negate_agent,
        target_not,
    })
}

/// What to do when a timeout fires.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum TimeoutAction {
    #[default]
    Kill,
    Warn,
}

/// Strategy for partitioning work across agents.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PartitionStrategy {
    Hash,
    RoundRobin,
    Directory,
}

/// An agent role with scope constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    /// Role ID, e.g. "r:impl"
    pub id: String,
    pub name: String,
    /// Glob patterns the agent CAN write to. Empty means everything allowed.
    pub allow_patterns: Vec<GlobPattern>,
    /// Glob patterns the agent CANNOT write to. Empty means nothing denied.
    pub deny_patterns: Vec<GlobPattern>,
}

/// A partition definition for deterministic work sharding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionDef {
    /// Partition ID, e.g. "p:1"
    pub id: String,
    /// Glob pattern for files in scope.
    pub scope_pattern: GlobPattern,
    pub strategy: PartitionStrategy,
    pub agent_count: u32,
}

/// Node annotation from extended @nodes (role=, scope=).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeAnnotation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// A behavioral edge between two nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaveEdge {
    pub from: String,
    pub to: String,
    pub edge_type: WeaveEdgeType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mutex_rule() {
        let rule = BThreadRule::parse("Mutex", "kind=FileWrite key=file:{target}").unwrap();
        match rule {
            BThreadRule::Mutex { scope, key, ttl_secs } => {
                assert_eq!(scope.kind, Some(EventKind::FileWrite));
                assert_eq!(key, "file:{target}");
                assert!(ttl_secs.is_none());
            }
            _ => panic!("Expected Mutex rule"),
        }
    }

    #[test]
    fn test_parse_mutex_rule_with_ttl() {
        let rule = BThreadRule::parse("Mutex", "kind=FileWrite key=file:{target} ttl=600").unwrap();
        match rule {
            BThreadRule::Mutex { ttl_secs, .. } => {
                assert_eq!(ttl_secs, Some(600));
            }
            _ => panic!("Expected Mutex rule"),
        }
    }

    #[test]
    fn test_parse_require_rule() {
        let rule = BThreadRule::parse("Require", "trigger=Commit prereq=TestPass").unwrap();
        match rule {
            BThreadRule::Require { trigger, prerequisite, reset } => {
                assert_eq!(trigger.kind, Some(EventKind::Commit));
                assert_eq!(prerequisite.kind, Some(EventKind::TestPass));
                assert!(reset.is_none());
            }
            _ => panic!("Expected Require rule"),
        }
    }

    #[test]
    fn test_parse_require_rule_with_reset() {
        let rule = BThreadRule::parse("Require", "trigger=Commit prereq=TestPass reset=FileWrite").unwrap();
        match rule {
            BThreadRule::Require { reset, .. } => {
                assert!(reset.is_some());
                assert_eq!(reset.unwrap().kind, Some(EventKind::FileWrite));
            }
            _ => panic!("Expected Require rule"),
        }
    }

    #[test]
    fn test_parse_block_always_rule() {
        let rule = BThreadRule::parse("BlockAlways", "kind=DangerousCommand").unwrap();
        match rule {
            BThreadRule::BlockAlways { scope } => {
                assert_eq!(scope.kind, Some(EventKind::DangerousCommand));
            }
            _ => panic!("Expected BlockAlways rule"),
        }
    }

    #[test]
    fn test_parse_block_until_rule() {
        let rule = BThreadRule::parse("BlockUntil", "trigger=ApiChange block=Build until=ApiReviewApproved").unwrap();
        match rule {
            BThreadRule::BlockUntil { trigger, block, until, escalate, escalation_message } => {
                assert_eq!(trigger.kind, Some(EventKind::ApiChange));
                assert_eq!(block.len(), 1);
                assert_eq!(block[0].kind, Some(EventKind::Build));
                assert_eq!(until.kind, Some(EventKind::Custom("ApiReviewApproved".to_string())));
                assert!(!escalate);
                assert!(escalation_message.is_none());
            }
            _ => panic!("Expected BlockUntil rule"),
        }
    }

    #[test]
    fn test_parse_block_until_with_escalate() {
        let rule = BThreadRule::parse("BlockUntil", "trigger=ApiChange block=Build until=Approved escalate=true").unwrap();
        match rule {
            BThreadRule::BlockUntil { escalate, .. } => {
                assert!(escalate);
            }
            _ => panic!("Expected BlockUntil rule"),
        }
    }

    #[test]
    fn test_parse_rate_limit_rule() {
        let rule = BThreadRule::parse("RateLimit", "kind=Commit max=5 window=120").unwrap();
        match rule {
            BThreadRule::RateLimit { scope, max, window_secs } => {
                assert_eq!(scope.kind, Some(EventKind::Commit));
                assert_eq!(max, 5);
                assert_eq!(window_secs, 120);
            }
            _ => panic!("Expected RateLimit rule"),
        }
    }

    #[test]
    fn test_parse_timeout_rule() {
        let rule = BThreadRule::parse("Timeout", "kind=TestRun max_secs=300 action=Warn").unwrap();
        match rule {
            BThreadRule::Timeout { scope, max_duration_secs, action } => {
                assert_eq!(scope.kind, Some(EventKind::TestRun));
                assert_eq!(max_duration_secs, 300);
                assert_eq!(action, TimeoutAction::Warn);
            }
            _ => panic!("Expected Timeout rule"),
        }
    }

    #[test]
    fn test_parse_timeout_rule_default_action() {
        let rule = BThreadRule::parse("Timeout", "kind=Build max_secs=600").unwrap();
        match rule {
            BThreadRule::Timeout { action, .. } => {
                assert_eq!(action, TimeoutAction::Kill);
            }
            _ => panic!("Expected Timeout rule"),
        }
    }

    #[test]
    fn test_parse_partition_rule_hash() {
        let rule = BThreadRule::parse("Partition", "kind=FileWrite count=4").unwrap();
        match rule {
            BThreadRule::Partition { scope, strategy, agent_count } => {
                assert_eq!(scope.kind, Some(EventKind::FileWrite));
                assert_eq!(strategy, PartitionStrategy::Hash);
                assert_eq!(agent_count, 4);
            }
            _ => panic!("Expected Partition rule"),
        }
    }

    #[test]
    fn test_parse_partition_rule_round_robin() {
        let rule = BThreadRule::parse("Partition", "kind=FileWrite strategy=round-robin count=3").unwrap();
        match rule {
            BThreadRule::Partition { strategy, .. } => {
                assert_eq!(strategy, PartitionStrategy::RoundRobin);
            }
            _ => panic!("Expected Partition rule"),
        }
    }

    #[test]
    fn test_parse_partition_rule_directory() {
        let rule = BThreadRule::parse("Partition", "kind=FileWrite strategy=Directory agent_count=2").unwrap();
        match rule {
            BThreadRule::Partition { strategy, agent_count, .. } => {
                assert_eq!(strategy, PartitionStrategy::Directory);
                assert_eq!(agent_count, 2);
            }
            _ => panic!("Expected Partition rule"),
        }
    }

    #[test]
    fn test_parse_unknown_rule_type_errors() {
        let result = BThreadRule::parse("UnknownRule", "kind=FileWrite");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown rule type"));
    }

    #[test]
    fn test_parse_mutex_missing_key_errors() {
        let result = BThreadRule::parse("Mutex", "kind=FileWrite");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("key"));
    }

    #[test]
    fn test_parse_require_missing_trigger_errors() {
        let result = BThreadRule::parse("Require", "prereq=TestPass");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("trigger"));
    }

    #[test]
    fn test_parse_require_missing_prereq_errors() {
        let result = BThreadRule::parse("Require", "trigger=Commit");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("prereq"));
    }

    #[test]
    fn test_parse_rate_limit_missing_max_errors() {
        let result = BThreadRule::parse("RateLimit", "kind=Commit window=60");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max"));
    }

    #[test]
    fn test_parse_rate_limit_missing_window_errors() {
        let result = BThreadRule::parse("RateLimit", "kind=Commit max=5");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("window"));
    }

    #[test]
    fn test_parse_timeout_missing_max_secs_errors() {
        let result = BThreadRule::parse("Timeout", "kind=TestRun action=kill");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_secs"));
    }

    #[test]
    fn test_parse_partition_missing_count_errors() {
        let result = BThreadRule::parse("Partition", "kind=FileWrite strategy=hash");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("count"));
    }

    #[test]
    fn test_parse_block_until_missing_trigger_errors() {
        let result = BThreadRule::parse("BlockUntil", "block=Build until=Approved");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("trigger"));
    }

    #[test]
    fn test_parse_block_until_missing_block_errors() {
        let result = BThreadRule::parse("BlockUntil", "trigger=ApiChange until=Approved");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("block"));
    }

    #[test]
    fn test_parse_block_until_missing_until_errors() {
        let result = BThreadRule::parse("BlockUntil", "trigger=ApiChange block=Build");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("until"));
    }

    #[test]
    fn test_parse_with_agent_and_target() {
        let rule = BThreadRule::parse("BlockAlways", "kind=FileWrite agent=bot target=src/**").unwrap();
        match rule {
            BThreadRule::BlockAlways { scope } => {
                assert_eq!(scope.kind, Some(EventKind::FileWrite));
                assert_eq!(scope.agent, Some("bot".to_string()));
                assert!(scope.target.is_some());
                assert_eq!(scope.target.unwrap().as_str(), "src/**");
            }
            _ => panic!("Expected BlockAlways rule"),
        }
    }

    #[test]
    fn test_parse_with_negate_agent() {
        let rule = BThreadRule::parse("BlockAlways", "kind=FileWrite agent=admin negate_agent=true").unwrap();
        match rule {
            BThreadRule::BlockAlways { scope } => {
                assert!(scope.negate_agent);
                assert_eq!(scope.agent, Some("admin".to_string()));
            }
            _ => panic!("Expected BlockAlways rule"),
        }
    }

    #[test]
    fn test_parse_with_target_not() {
        let rule = BThreadRule::parse("Mutex", "kind=FileWrite key=f:{target} target_not=docs/**,*.md").unwrap();
        match rule {
            BThreadRule::Mutex { scope, .. } => {
                assert_eq!(scope.target_not.len(), 2);
                assert_eq!(scope.target_not[0].as_str(), "docs/**");
                assert_eq!(scope.target_not[1].as_str(), "*.md");
            }
            _ => panic!("Expected Mutex rule"),
        }
    }

    #[test]
    fn test_parse_block_until_multiple_block_kinds() {
        let rule = BThreadRule::parse("BlockUntil", "trigger=ApiChange block=Build,Commit until=Approved").unwrap();
        match rule {
            BThreadRule::BlockUntil { block, .. } => {
                assert_eq!(block.len(), 2);
                assert_eq!(block[0].kind, Some(EventKind::Build));
                assert_eq!(block[1].kind, Some(EventKind::Commit));
            }
            _ => panic!("Expected BlockUntil rule"),
        }
    }

    #[test]
    fn test_parse_key_values_helper() {
        let params = parse_key_values("kind=FileWrite key=file:{target} ttl=600");
        assert_eq!(params.get("kind").unwrap(), "FileWrite");
        assert_eq!(params.get("key").unwrap(), "file:{target}");
        assert_eq!(params.get("ttl").unwrap(), "600");
    }

    #[test]
    fn test_parse_key_values_empty() {
        let params = parse_key_values("");
        assert!(params.is_empty());
    }

    #[test]
    fn test_parse_key_values_ignores_bare_tokens() {
        let params = parse_key_values("kind=FileWrite baretoken key=val");
        assert_eq!(params.len(), 2);
        assert!(!params.contains_key("baretoken"));
    }

    #[test]
    fn test_weave_edge_type_parse() {
        assert_eq!(WeaveEdgeType::parse("~~"), Some(WeaveEdgeType::Conflict));
        assert_eq!(WeaveEdgeType::parse(">>"), Some(WeaveEdgeType::Sequence));
        assert_eq!(WeaveEdgeType::parse("!="), Some(WeaveEdgeType::Exclusion));
        assert_eq!(WeaveEdgeType::parse("->"), None);
        assert_eq!(WeaveEdgeType::parse("xx"), None);
    }

    #[test]
    fn test_weave_edge_type_operator_roundtrip() {
        for edge_type in [WeaveEdgeType::Conflict, WeaveEdgeType::Sequence, WeaveEdgeType::Exclusion] {
            let op = edge_type.operator();
            assert_eq!(WeaveEdgeType::parse(op), Some(edge_type));
        }
    }
}

/// Type of behavioral edge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WeaveEdgeType {
    /// `~~` — tasks conflict if run simultaneously
    Conflict,
    /// `>>` — behavioral ordering (no data dependency)
    Sequence,
    /// `!=` — must NOT run on same agent
    Exclusion,
}

impl WeaveEdgeType {
    pub fn operator(&self) -> &'static str {
        match self {
            WeaveEdgeType::Conflict => "~~",
            WeaveEdgeType::Sequence => ">>",
            WeaveEdgeType::Exclusion => "!=",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "~~" => Some(WeaveEdgeType::Conflict),
            ">>" => Some(WeaveEdgeType::Sequence),
            "!=" => Some(WeaveEdgeType::Exclusion),
            _ => None,
        }
    }
}
