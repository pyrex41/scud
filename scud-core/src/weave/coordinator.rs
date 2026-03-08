//! Coordinator — evaluates b-thread rules against proposed events.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use super::bthread::{BThread, BThreadRule, PartitionDef, PartitionStrategy, Role};
use super::event::Event;
use super::log::TimestampedEvent;

/// An active mutex lock held by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveLock {
    pub lock_key: String,
    pub holder_agent: String,
    pub task_id: Option<String>,
    pub acquired_at: String,
    pub ttl_secs: u64,
}

impl ActiveLock {
    /// Check if this lock has expired based on current time.
    pub fn is_expired(&self) -> bool {
        if self.ttl_secs == 0 {
            return false;
        }
        let Ok(acquired) = chrono::DateTime::parse_from_rfc3339(&self.acquired_at) else {
            return false;
        };
        let elapsed = Utc::now().signed_duration_since(acquired);
        elapsed.num_seconds() as u64 >= self.ttl_secs
    }
}

/// The coordinator's decision for a proposed event.
#[derive(Debug, Clone)]
pub enum Decision {
    /// Event is allowed to proceed.
    Proceed,
    /// Event should wait (prerequisite not yet satisfied).
    Wait {
        reason: String,
        thread_id: String,
    },
    /// Event is blocked (unconditional or resource conflict).
    Blocked {
        reason: String,
        thread_id: String,
    },
}

impl Decision {
    pub fn is_proceed(&self) -> bool {
        matches!(self, Decision::Proceed)
    }

    pub fn is_blocked(&self) -> bool {
        matches!(self, Decision::Blocked { .. })
    }

    pub fn is_wait(&self) -> bool {
        matches!(self, Decision::Wait { .. })
    }
}

impl std::fmt::Display for Decision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Decision::Proceed => write!(f, "PROCEED"),
            Decision::Wait { reason, thread_id } => {
                write!(f, "WAIT for {} \"{}\"", thread_id, reason)
            }
            Decision::Blocked { reason, thread_id } => {
                write!(f, "BLOCKED by {} \"{}\"", thread_id, reason)
            }
        }
    }
}

/// The b-thread coordinator.
pub struct Coordinator {
    pub threads: Vec<BThread>,
    pub roles: Vec<Role>,
    pub partitions: Vec<PartitionDef>,
    pub active_locks: HashMap<String, ActiveLock>,
    pub event_log: Vec<TimestampedEvent>,
}

impl Coordinator {
    /// Create an empty coordinator.
    pub fn new() -> Self {
        Coordinator {
            threads: Vec::new(),
            roles: Vec::new(),
            partitions: Vec::new(),
            active_locks: HashMap::new(),
            event_log: Vec::new(),
        }
    }

    /// Expire any locks that have exceeded their TTL.
    pub fn expire_locks(&mut self) {
        self.active_locks.retain(|_, lock| !lock.is_expired());
    }

    /// Core evaluation: given a proposed event, check all b-threads.
    ///
    /// Evaluation order (by b-thread priority, then rule type):
    /// BlockAlways -> Role check -> Partition -> Mutex -> Require -> BlockUntil -> RateLimit -> Timeout
    ///
    /// Returns the first blocking/waiting decision found.
    pub fn evaluate(&self, event: &Event) -> Decision {
        // Sort threads by priority (lower = higher priority)
        let mut sorted: Vec<&BThread> = self.threads.iter().filter(|t| t.enabled).collect();
        sorted.sort_by_key(|t| t.priority);

        for thread in &sorted {
            for rule in &thread.rules {
                let decision = self.evaluate_rule(rule, event, &thread.id, &thread.name);
                if !decision.is_proceed() {
                    return decision;
                }
            }
        }

        Decision::Proceed
    }

    /// Evaluate a single rule against an event.
    fn evaluate_rule(
        &self,
        rule: &BThreadRule,
        event: &Event,
        thread_id: &str,
        thread_name: &str,
    ) -> Decision {
        match rule {
            BThreadRule::BlockAlways { scope } => {
                if scope.matches_event(event) {
                    return Decision::Blocked {
                        reason: format!("{}: unconditionally blocked", thread_name),
                        thread_id: thread_id.to_string(),
                    };
                }
            }
            BThreadRule::Mutex {
                scope,
                key,
                ttl_secs: _,
            } => {
                if scope.matches_event(event) {
                    let expanded_key = expand_key_template(key, event);
                    if let Some(lock) = self.active_locks.get(&expanded_key) {
                        if !lock.is_expired() {
                            // Blocked if different agent holds lock
                            let event_agent = event.agent.as_deref().unwrap_or("");
                            if lock.holder_agent != event_agent {
                                return Decision::Blocked {
                                    reason: format!(
                                        "{}: {} holds {} ({})",
                                        thread_name,
                                        lock.holder_agent,
                                        expanded_key,
                                        lock.task_id.as_deref().unwrap_or("no task"),
                                    ),
                                    thread_id: thread_id.to_string(),
                                };
                            }
                        }
                    }
                }
            }
            BThreadRule::Require {
                trigger,
                prerequisite,
                reset,
            } => {
                if trigger.matches_event(event) {
                    // Find the last reset event (if reset pattern specified)
                    let reset_idx = reset.as_ref().and_then(|rp| {
                        self.event_log
                            .iter()
                            .rposition(|te| rp.matches_event(&te.event))
                    });

                    // Check if prerequisite exists after the last reset
                    let search_from = reset_idx.map(|i| i + 1).unwrap_or(0);
                    let has_prereq = self.event_log[search_from..]
                        .iter()
                        .any(|te| prerequisite.matches_event(&te.event));

                    if !has_prereq {
                        return Decision::Wait {
                            reason: format!(
                                "{}: {} required",
                                thread_name,
                                prerequisite
                                    .kind
                                    .as_ref()
                                    .map(|k| k.as_str())
                                    .unwrap_or("prerequisite"),
                            ),
                            thread_id: thread_id.to_string(),
                        };
                    }
                }
            }
            BThreadRule::BlockUntil {
                trigger,
                block,
                until,
                ..
            } => {
                // Check if trigger has been seen
                let trigger_seen = self
                    .event_log
                    .iter()
                    .rposition(|te| trigger.matches_event(&te.event));

                if let Some(trigger_idx) = trigger_seen {
                    // Check if "until" event has been seen after trigger
                    let until_seen = self.event_log[trigger_idx..]
                        .iter()
                        .any(|te| until.matches_event(&te.event));

                    if !until_seen {
                        // We're in the "triggered but not yet resolved" state
                        // Check if the current event matches any of the block patterns
                        for bp in block {
                            if bp.matches_event(event) {
                                return Decision::Blocked {
                                    reason: format!(
                                        "{}: blocked until {}",
                                        thread_name,
                                        until
                                            .kind
                                            .as_ref()
                                            .map(|k| k.as_str())
                                            .unwrap_or("condition met"),
                                    ),
                                    thread_id: thread_id.to_string(),
                                };
                            }
                        }
                    }
                }
            }
            BThreadRule::RateLimit {
                scope,
                max,
                window_secs,
            } => {
                if scope.matches_event(event) {
                    let now = Utc::now();
                    let count = self
                        .event_log
                        .iter()
                        .filter(|te| {
                            if !scope.matches_event(&te.event) {
                                return false;
                            }
                            if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&te.timestamp) {
                                let elapsed = now.signed_duration_since(ts);
                                elapsed.num_seconds() < *window_secs as i64
                            } else {
                                false
                            }
                        })
                        .count() as u32;

                    if count >= *max {
                        return Decision::Blocked {
                            reason: format!(
                                "{}: rate limit exceeded ({}/{} in {}s window)",
                                thread_name, count, max, window_secs,
                            ),
                            thread_id: thread_id.to_string(),
                        };
                    }
                }
            }
            BThreadRule::Timeout {
                scope,
                max_duration_secs,
                action: _,
            } => {
                if scope.matches_event(event) {
                    // Check if there's a matching event that started too long ago
                    let now = Utc::now();
                    for te in self.event_log.iter().rev() {
                        if scope.matches_event(&te.event) {
                            if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&te.timestamp) {
                                let elapsed = now.signed_duration_since(ts);
                                if elapsed.num_seconds() as u64 > *max_duration_secs {
                                    return Decision::Blocked {
                                        reason: format!(
                                            "{}: timeout exceeded ({}s > {}s max)",
                                            thread_name,
                                            elapsed.num_seconds(),
                                            max_duration_secs,
                                        ),
                                        thread_id: thread_id.to_string(),
                                    };
                                }
                            }
                            break;
                        }
                    }
                }
            }
            BThreadRule::Partition {
                scope,
                strategy,
                agent_count,
            } => {
                if scope.matches_event(event) {
                    if let (Some(agent), Some(target)) = (&event.agent, &event.target) {
                        let agent_slot = parse_agent_slot(agent);
                        let target_slot = compute_partition_slot(target, *strategy, *agent_count);
                        if agent_slot != target_slot {
                            return Decision::Blocked {
                                reason: format!(
                                    "{}: {} assigned to slot {}, target in slot {}",
                                    thread_name, agent, agent_slot, target_slot,
                                ),
                                thread_id: thread_id.to_string(),
                            };
                        }
                    }
                }
            }
        }

        // Also check role constraints (outside b-threads)
        // Role checks are done in evaluate() directly
        Decision::Proceed
    }

    /// Check role constraints for an event.
    fn check_roles(&self, event: &Event) -> Decision {
        // Role checks require an agent and a target
        let Some(_agent) = &event.agent else {
            return Decision::Proceed;
        };
        let Some(target) = &event.target else {
            return Decision::Proceed;
        };

        // Find agent's role from task node annotations (stored in metadata)
        let role_id = event.metadata.get("role");
        let Some(role_id) = role_id else {
            return Decision::Proceed;
        };

        // Find the role definition
        let Some(role) = self.roles.iter().find(|r| r.id == *role_id) else {
            return Decision::Proceed;
        };

        // Check deny patterns first
        for deny in &role.deny_patterns {
            if deny.matches(target) {
                return Decision::Blocked {
                    reason: format!(
                        "role \"{}\": {} denied for {} role",
                        role.name, target, role.name,
                    ),
                    thread_id: format!("role:{}", role.id),
                };
            }
        }

        // If allow patterns exist and are non-empty, target must match at least one
        if !role.allow_patterns.is_empty() {
            let allowed = role.allow_patterns.iter().any(|p| p.matches(target));
            if !allowed {
                return Decision::Blocked {
                    reason: format!(
                        "role \"{}\": {} not in allowed scope for {} role",
                        role.name, target, role.name,
                    ),
                    thread_id: format!("role:{}", role.id),
                };
            }
        }

        Decision::Proceed
    }

    /// Full evaluation including role checks.
    pub fn evaluate_full(&self, event: &Event) -> Decision {
        // Check roles first
        let role_decision = self.check_roles(event);
        if !role_decision.is_proceed() {
            return role_decision;
        }

        self.evaluate(event)
    }

    /// Record that an event occurred. Updates locks, appends to event log.
    pub fn record(&mut self, event: &Event) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();

        // Append to event log
        self.event_log.push(TimestampedEvent {
            timestamp: now.clone(),
            event: event.clone(),
        });

        // Check for mutex rules — create locks for matching events
        for thread in &self.threads {
            if !thread.enabled {
                continue;
            }
            for rule in &thread.rules {
                if let BThreadRule::Mutex {
                    scope,
                    key,
                    ttl_secs,
                } = rule
                {
                    if scope.matches_event(event) {
                        let expanded_key = expand_key_template(key, event);
                        if let Some(agent) = &event.agent {
                            self.active_locks.insert(
                                expanded_key.clone(),
                                ActiveLock {
                                    lock_key: expanded_key,
                                    holder_agent: agent.clone(),
                                    task_id: event.task_id.clone(),
                                    acquired_at: now.clone(),
                                    ttl_secs: ttl_secs.unwrap_or(3600),
                                },
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Release a specific mutex lock.
    pub fn release(&mut self, lock_key: &str, _agent: &str) -> anyhow::Result<()> {
        self.active_locks.remove(lock_key);
        Ok(())
    }

    /// Release all locks held by an agent, optionally filtered by task.
    pub fn release_all(&mut self, agent: &str, task_id: Option<&str>) -> anyhow::Result<()> {
        self.active_locks.retain(|_, lock| {
            if lock.holder_agent != agent {
                return true;
            }
            if let Some(tid) = task_id {
                lock.task_id.as_deref() != Some(tid)
            } else {
                false
            }
        });
        Ok(())
    }
}

impl Default for Coordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// Expand a key template by replacing `{target}` and `{agent}` with event values.
fn expand_key_template(template: &str, event: &Event) -> String {
    let mut result = template.to_string();
    if let Some(ref target) = event.target {
        result = result.replace("{target}", target);
    }
    if let Some(ref agent) = event.agent {
        result = result.replace("{agent}", agent);
    }
    result
}

/// Parse an agent slot number from an agent name like "agent-0", "agent-1".
fn parse_agent_slot(agent: &str) -> u32 {
    agent
        .rsplit('-')
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0)
}

/// Compute which partition slot a target belongs to.
fn compute_partition_slot(target: &str, strategy: PartitionStrategy, agent_count: u32) -> u32 {
    if agent_count == 0 {
        return 0;
    }
    match strategy {
        PartitionStrategy::Hash => {
            let mut hasher = DefaultHasher::new();
            target.hash(&mut hasher);
            (hasher.finish() % agent_count as u64) as u32
        }
        PartitionStrategy::RoundRobin => {
            // Sort-order based: hash for determinism
            let mut hasher = DefaultHasher::new();
            target.hash(&mut hasher);
            (hasher.finish() % agent_count as u64) as u32
        }
        PartitionStrategy::Directory => {
            // Use top-level directory
            let dir = target.split('/').next().unwrap_or(target);
            let mut hasher = DefaultHasher::new();
            dir.hash(&mut hasher);
            (hasher.finish() % agent_count as u64) as u32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weave::event::{EventKind, EventPattern};
    use crate::weave::matcher::GlobPattern;

    fn make_event(kind: EventKind, agent: &str, target: &str) -> Event {
        Event {
            kind,
            agent: Some(agent.to_string()),
            target: Some(target.to_string()),
            task_id: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    fn make_timestamped(event: Event, timestamp: &str) -> TimestampedEvent {
        TimestampedEvent {
            timestamp: timestamp.to_string(),
            event,
        }
    }

    #[test]
    fn test_block_always() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:1".to_string(),
            name: "No dangerous".to_string(),
            priority: 1,
            enabled: true,
            rules: vec![BThreadRule::BlockAlways {
                scope: EventPattern::kind(EventKind::DangerousCommand),
            }],
        });

        let event = make_event(EventKind::DangerousCommand, "agent-1", "pkill bash");
        assert!(coord.evaluate(&event).is_blocked());

        let event = make_event(EventKind::FileWrite, "agent-1", "src/main.rs");
        assert!(coord.evaluate(&event).is_proceed());
    }

    #[test]
    fn test_mutex_different_agents() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:1".to_string(),
            name: "File mutex".to_string(),
            priority: 5,
            enabled: true,
            rules: vec![BThreadRule::Mutex {
                scope: EventPattern::kind(EventKind::FileWrite),
                key: "file:{target}".to_string(),
                ttl_secs: Some(3600),
            }],
        });

        // agent-1 holds lock
        coord.active_locks.insert(
            "file:src/main.rs".to_string(),
            ActiveLock {
                lock_key: "file:src/main.rs".to_string(),
                holder_agent: "agent-1".to_string(),
                task_id: Some("auth:1".to_string()),
                acquired_at: Utc::now().to_rfc3339(),
                ttl_secs: 3600,
            },
        );

        // agent-2 tries to write same file -> BLOCKED
        let event = make_event(EventKind::FileWrite, "agent-2", "src/main.rs");
        assert!(coord.evaluate(&event).is_blocked());

        // agent-1 writes same file -> PROCEED (same agent)
        let event = make_event(EventKind::FileWrite, "agent-1", "src/main.rs");
        assert!(coord.evaluate(&event).is_proceed());

        // agent-2 writes different file -> PROCEED
        let event = make_event(EventKind::FileWrite, "agent-2", "src/lib.rs");
        assert!(coord.evaluate(&event).is_proceed());
    }

    #[test]
    fn test_require_no_prereq() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:2".to_string(),
            name: "Test gate".to_string(),
            priority: 10,
            enabled: true,
            rules: vec![BThreadRule::Require {
                trigger: EventPattern::kind(EventKind::Commit),
                prerequisite: EventPattern::kind(EventKind::TestPass),
                reset: Some(EventPattern::kind(EventKind::FileWrite)),
            }],
        });

        // No events logged yet, try to commit -> WAIT
        let event = make_event(EventKind::Commit, "agent-1", "");
        assert!(coord.evaluate(&event).is_wait());
    }

    #[test]
    fn test_require_with_prereq() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:2".to_string(),
            name: "Test gate".to_string(),
            priority: 10,
            enabled: true,
            rules: vec![BThreadRule::Require {
                trigger: EventPattern::kind(EventKind::Commit),
                prerequisite: EventPattern::kind(EventKind::TestPass),
                reset: Some(EventPattern::kind(EventKind::FileWrite)),
            }],
        });

        // TestPass logged
        coord.event_log.push(make_timestamped(
            make_event(EventKind::TestPass, "agent-1", ""),
            &Utc::now().to_rfc3339(),
        ));

        // Commit -> PROCEED
        let event = make_event(EventKind::Commit, "agent-1", "");
        assert!(coord.evaluate(&event).is_proceed());
    }

    #[test]
    fn test_require_reset() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:2".to_string(),
            name: "Test gate".to_string(),
            priority: 10,
            enabled: true,
            rules: vec![BThreadRule::Require {
                trigger: EventPattern::kind(EventKind::Commit),
                prerequisite: EventPattern::kind(EventKind::TestPass),
                reset: Some(EventPattern::kind(EventKind::FileWrite)),
            }],
        });

        let now = Utc::now();
        // TestPass
        coord.event_log.push(make_timestamped(
            make_event(EventKind::TestPass, "agent-1", ""),
            &now.to_rfc3339(),
        ));
        // FileWrite after TestPass -> resets requirement
        coord.event_log.push(make_timestamped(
            make_event(EventKind::FileWrite, "agent-1", "src/main.rs"),
            &(now + chrono::Duration::seconds(1)).to_rfc3339(),
        ));

        // Commit -> WAIT (FileWrite reset the prerequisite)
        let event = make_event(EventKind::Commit, "agent-1", "");
        assert!(coord.evaluate(&event).is_wait());
    }

    #[test]
    fn test_disabled_thread_ignored() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:1".to_string(),
            name: "Disabled".to_string(),
            priority: 1,
            enabled: false,
            rules: vec![BThreadRule::BlockAlways {
                scope: EventPattern::kind(EventKind::DangerousCommand),
            }],
        });

        let event = make_event(EventKind::DangerousCommand, "agent-1", "pkill");
        assert!(coord.evaluate(&event).is_proceed());
    }

    #[test]
    fn test_record_creates_locks() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:1".to_string(),
            name: "File mutex".to_string(),
            priority: 5,
            enabled: true,
            rules: vec![BThreadRule::Mutex {
                scope: EventPattern::kind(EventKind::FileWrite),
                key: "file:{target}".to_string(),
                ttl_secs: Some(3600),
            }],
        });

        let event = make_event(EventKind::FileWrite, "agent-1", "src/main.rs");
        coord.record(&event).unwrap();

        assert!(coord.active_locks.contains_key("file:src/main.rs"));
        assert_eq!(
            coord.active_locks["file:src/main.rs"].holder_agent,
            "agent-1"
        );
        assert_eq!(coord.event_log.len(), 1);
    }

    #[test]
    fn test_role_deny() {
        let mut coord = Coordinator::new();
        coord.roles.push(Role {
            id: "r:impl".to_string(),
            name: "Implementer".to_string(),
            allow_patterns: vec![GlobPattern::new("src/**")],
            deny_patterns: vec![GlobPattern::new("docs/**")],
        });

        let mut event = make_event(EventKind::FileWrite, "agent-1", "docs/readme.md");
        event.metadata.insert("role".to_string(), "r:impl".to_string());

        assert!(coord.check_roles(&event).is_blocked());
    }

    #[test]
    fn test_role_allow() {
        let mut coord = Coordinator::new();
        coord.roles.push(Role {
            id: "r:impl".to_string(),
            name: "Implementer".to_string(),
            allow_patterns: vec![GlobPattern::new("src/**")],
            deny_patterns: vec![GlobPattern::new("docs/**")],
        });

        let mut event = make_event(EventKind::FileWrite, "agent-1", "src/main.rs");
        event.metadata.insert("role".to_string(), "r:impl".to_string());

        assert!(coord.check_roles(&event).is_proceed());
    }

    #[test]
    fn test_priority_ordering() {
        let mut coord = Coordinator::new();
        // Lower priority (higher number) should not be checked if higher priority blocks
        coord.threads.push(BThread {
            id: "w:1".to_string(),
            name: "High priority block".to_string(),
            priority: 1,
            enabled: true,
            rules: vec![BThreadRule::BlockAlways {
                scope: EventPattern::kind(EventKind::DangerousCommand),
            }],
        });
        coord.threads.push(BThread {
            id: "w:2".to_string(),
            name: "Lower priority".to_string(),
            priority: 50,
            enabled: true,
            rules: vec![BThreadRule::BlockAlways {
                scope: EventPattern::kind(EventKind::DangerousCommand),
            }],
        });

        let event = make_event(EventKind::DangerousCommand, "agent-1", "pkill");
        let decision = coord.evaluate(&event);
        match &decision {
            Decision::Blocked { thread_id, .. } => assert_eq!(thread_id, "w:1"),
            _ => panic!("Expected blocked"),
        }
    }

    #[test]
    fn test_expand_key_template() {
        let event = make_event(EventKind::FileWrite, "agent-1", "src/main.rs");
        assert_eq!(expand_key_template("file:{target}", &event), "file:src/main.rs");
        assert_eq!(expand_key_template("{agent}-lock", &event), "agent-1-lock");
        assert_eq!(expand_key_template("schema-global", &event), "schema-global");
    }

    #[test]
    fn test_rate_limit_under_limit_proceeds() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:10".to_string(),
            name: "Commit rate".to_string(),
            priority: 50,
            enabled: true,
            rules: vec![BThreadRule::RateLimit {
                scope: EventPattern::kind(EventKind::Commit),
                max: 3,
                window_secs: 300,
            }],
        });

        // Log 2 events (under limit of 3)
        let now = Utc::now();
        coord.event_log.push(make_timestamped(
            make_event(EventKind::Commit, "agent-1", ""),
            &now.to_rfc3339(),
        ));
        coord.event_log.push(make_timestamped(
            make_event(EventKind::Commit, "agent-1", ""),
            &now.to_rfc3339(),
        ));

        let event = make_event(EventKind::Commit, "agent-1", "");
        assert!(coord.evaluate(&event).is_proceed());
    }

    #[test]
    fn test_rate_limit_over_limit_blocked() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:10".to_string(),
            name: "Commit rate".to_string(),
            priority: 50,
            enabled: true,
            rules: vec![BThreadRule::RateLimit {
                scope: EventPattern::kind(EventKind::Commit),
                max: 2,
                window_secs: 300,
            }],
        });

        // Log 2 events (at limit)
        let now = Utc::now();
        for _ in 0..2 {
            coord.event_log.push(make_timestamped(
                make_event(EventKind::Commit, "agent-1", ""),
                &now.to_rfc3339(),
            ));
        }

        let event = make_event(EventKind::Commit, "agent-1", "");
        let decision = coord.evaluate(&event);
        assert!(decision.is_blocked());
        match decision {
            Decision::Blocked { reason, .. } => {
                assert!(reason.contains("rate limit"));
            }
            _ => panic!("Expected blocked"),
        }
    }

    #[test]
    fn test_rate_limit_old_events_not_counted() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:10".to_string(),
            name: "Commit rate".to_string(),
            priority: 50,
            enabled: true,
            rules: vec![BThreadRule::RateLimit {
                scope: EventPattern::kind(EventKind::Commit),
                max: 2,
                window_secs: 60,
            }],
        });

        // Log events from 2 hours ago (outside 60s window)
        let old = Utc::now() - chrono::Duration::hours(2);
        for _ in 0..5 {
            coord.event_log.push(make_timestamped(
                make_event(EventKind::Commit, "agent-1", ""),
                &old.to_rfc3339(),
            ));
        }

        let event = make_event(EventKind::Commit, "agent-1", "");
        assert!(coord.evaluate(&event).is_proceed());
    }

    #[test]
    fn test_partition_in_slot_proceeds() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:11".to_string(),
            name: "File partition".to_string(),
            priority: 20,
            enabled: true,
            rules: vec![BThreadRule::Partition {
                scope: EventPattern::kind(EventKind::FileWrite),
                strategy: PartitionStrategy::Hash,
                agent_count: 2,
            }],
        });

        // We need to find a target that hashes to agent-0's slot (slot 0)
        // Try different targets until we find one in slot 0
        let target = find_target_for_slot(0, PartitionStrategy::Hash, 2);
        let event = make_event(EventKind::FileWrite, "agent-0", &target);
        assert!(coord.evaluate(&event).is_proceed());
    }

    #[test]
    fn test_partition_out_of_slot_blocked() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:11".to_string(),
            name: "File partition".to_string(),
            priority: 20,
            enabled: true,
            rules: vec![BThreadRule::Partition {
                scope: EventPattern::kind(EventKind::FileWrite),
                strategy: PartitionStrategy::Hash,
                agent_count: 2,
            }],
        });

        // Find a target for slot 1, then try with agent-0 (slot 0) -> blocked
        let target = find_target_for_slot(1, PartitionStrategy::Hash, 2);
        let event = make_event(EventKind::FileWrite, "agent-0", &target);
        assert!(coord.evaluate(&event).is_blocked());
    }

    // Helper: find a target string whose hash maps to a specific slot
    fn find_target_for_slot(slot: u32, strategy: PartitionStrategy, count: u32) -> String {
        for i in 0..1000 {
            let target = format!("src/file{}.rs", i);
            if compute_partition_slot(&target, strategy, count) == slot {
                return target;
            }
        }
        panic!("Could not find target for slot {}", slot);
    }

    #[test]
    fn test_timeout_expired_operation() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:12".to_string(),
            name: "Build timeout".to_string(),
            priority: 30,
            enabled: true,
            rules: vec![BThreadRule::Timeout {
                scope: EventPattern::kind(EventKind::Build),
                max_duration_secs: 60,
                action: super::super::bthread::TimeoutAction::Kill,
            }],
        });

        // Log a build event from 2 minutes ago (exceeds 60s timeout)
        let old = Utc::now() - chrono::Duration::minutes(2);
        coord.event_log.push(make_timestamped(
            make_event(EventKind::Build, "agent-1", ""),
            &old.to_rfc3339(),
        ));

        let event = make_event(EventKind::Build, "agent-1", "");
        let decision = coord.evaluate(&event);
        assert!(decision.is_blocked());
        match decision {
            Decision::Blocked { reason, .. } => {
                assert!(reason.contains("timeout"));
            }
            _ => panic!("Expected blocked"),
        }
    }

    #[test]
    fn test_timeout_not_yet_expired() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:12".to_string(),
            name: "Build timeout".to_string(),
            priority: 30,
            enabled: true,
            rules: vec![BThreadRule::Timeout {
                scope: EventPattern::kind(EventKind::Build),
                max_duration_secs: 3600,
                action: super::super::bthread::TimeoutAction::Kill,
            }],
        });

        // Log a build event from just now (well within 3600s)
        coord.event_log.push(make_timestamped(
            make_event(EventKind::Build, "agent-1", ""),
            &Utc::now().to_rfc3339(),
        ));

        let event = make_event(EventKind::Build, "agent-1", "");
        assert!(coord.evaluate(&event).is_proceed());
    }

    #[test]
    fn test_ttl_expiry_auto_releases() {
        let mut coord = Coordinator::new();
        // Insert a lock that expired 2 hours ago (ttl=1s, acquired 2h ago)
        let old = Utc::now() - chrono::Duration::hours(2);
        coord.active_locks.insert(
            "file:src/main.rs".to_string(),
            ActiveLock {
                lock_key: "file:src/main.rs".to_string(),
                holder_agent: "agent-1".to_string(),
                task_id: None,
                acquired_at: old.to_rfc3339(),
                ttl_secs: 1,
            },
        );

        coord.expire_locks();
        assert!(coord.active_locks.is_empty());
    }

    #[test]
    fn test_ttl_zero_never_expires() {
        let mut coord = Coordinator::new();
        let old = Utc::now() - chrono::Duration::hours(2);
        coord.active_locks.insert(
            "global".to_string(),
            ActiveLock {
                lock_key: "global".to_string(),
                holder_agent: "agent-1".to_string(),
                task_id: None,
                acquired_at: old.to_rfc3339(),
                ttl_secs: 0,
            },
        );

        coord.expire_locks();
        assert_eq!(coord.active_locks.len(), 1);
    }

    #[test]
    fn test_release_specific_lock() {
        let mut coord = Coordinator::new();
        coord.active_locks.insert(
            "file:a.rs".to_string(),
            ActiveLock {
                lock_key: "file:a.rs".to_string(),
                holder_agent: "agent-1".to_string(),
                task_id: None,
                acquired_at: Utc::now().to_rfc3339(),
                ttl_secs: 3600,
            },
        );
        coord.active_locks.insert(
            "file:b.rs".to_string(),
            ActiveLock {
                lock_key: "file:b.rs".to_string(),
                holder_agent: "agent-1".to_string(),
                task_id: None,
                acquired_at: Utc::now().to_rfc3339(),
                ttl_secs: 3600,
            },
        );

        coord.release("file:a.rs", "agent-1").unwrap();
        assert!(!coord.active_locks.contains_key("file:a.rs"));
        assert!(coord.active_locks.contains_key("file:b.rs"));
    }

    #[test]
    fn test_release_all_by_agent() {
        let mut coord = Coordinator::new();
        for key in ["file:a.rs", "file:b.rs"] {
            coord.active_locks.insert(
                key.to_string(),
                ActiveLock {
                    lock_key: key.to_string(),
                    holder_agent: "agent-1".to_string(),
                    task_id: None,
                    acquired_at: Utc::now().to_rfc3339(),
                    ttl_secs: 3600,
                },
            );
        }
        coord.active_locks.insert(
            "file:c.rs".to_string(),
            ActiveLock {
                lock_key: "file:c.rs".to_string(),
                holder_agent: "agent-2".to_string(),
                task_id: None,
                acquired_at: Utc::now().to_rfc3339(),
                ttl_secs: 3600,
            },
        );

        coord.release_all("agent-1", None).unwrap();
        assert_eq!(coord.active_locks.len(), 1);
        assert!(coord.active_locks.contains_key("file:c.rs"));
    }

    #[test]
    fn test_release_all_by_agent_and_task() {
        let mut coord = Coordinator::new();
        coord.active_locks.insert(
            "file:a.rs".to_string(),
            ActiveLock {
                lock_key: "file:a.rs".to_string(),
                holder_agent: "agent-1".to_string(),
                task_id: Some("task-1".to_string()),
                acquired_at: Utc::now().to_rfc3339(),
                ttl_secs: 3600,
            },
        );
        coord.active_locks.insert(
            "file:b.rs".to_string(),
            ActiveLock {
                lock_key: "file:b.rs".to_string(),
                holder_agent: "agent-1".to_string(),
                task_id: Some("task-2".to_string()),
                acquired_at: Utc::now().to_rfc3339(),
                ttl_secs: 3600,
            },
        );

        // Only release locks for task-1
        coord.release_all("agent-1", Some("task-1")).unwrap();
        assert_eq!(coord.active_locks.len(), 1);
        assert!(coord.active_locks.contains_key("file:b.rs"));
    }

    #[test]
    fn test_multiple_threads_first_block_wins() {
        let mut coord = Coordinator::new();
        // Thread with priority 5 blocks FileWrite
        coord.threads.push(BThread {
            id: "w:1".to_string(),
            name: "File block".to_string(),
            priority: 5,
            enabled: true,
            rules: vec![BThreadRule::BlockAlways {
                scope: EventPattern::kind(EventKind::FileWrite),
            }],
        });
        // Thread with priority 10 also blocks FileWrite (but lower priority)
        coord.threads.push(BThread {
            id: "w:2".to_string(),
            name: "Also file block".to_string(),
            priority: 10,
            enabled: true,
            rules: vec![BThreadRule::BlockAlways {
                scope: EventPattern::kind(EventKind::FileWrite),
            }],
        });

        let event = make_event(EventKind::FileWrite, "agent-1", "src/main.rs");
        let decision = coord.evaluate(&event);
        match &decision {
            Decision::Blocked { thread_id, .. } => {
                assert_eq!(thread_id, "w:1", "Higher priority (lower number) thread should block first");
            }
            _ => panic!("Expected blocked"),
        }
    }

    #[test]
    fn test_evaluate_full_role_check_before_threads() {
        let mut coord = Coordinator::new();
        coord.roles.push(Role {
            id: "r:test".to_string(),
            name: "Tester".to_string(),
            allow_patterns: vec![GlobPattern::new("tests/**")],
            deny_patterns: vec![],
        });

        // Also add a thread that would block
        coord.threads.push(BThread {
            id: "w:1".to_string(),
            name: "Always block".to_string(),
            priority: 1,
            enabled: true,
            rules: vec![BThreadRule::BlockAlways {
                scope: EventPattern::kind(EventKind::FileWrite),
            }],
        });

        let mut event = make_event(EventKind::FileWrite, "agent-1", "src/main.rs");
        event.metadata.insert("role".to_string(), "r:test".to_string());

        // Role check should block before b-thread evaluation
        let decision = coord.evaluate_full(&event);
        assert!(decision.is_blocked());
        match decision {
            Decision::Blocked { thread_id, .. } => {
                assert!(thread_id.starts_with("role:"), "Role should block, not thread");
            }
            _ => panic!("Expected blocked"),
        }
    }

    #[test]
    fn test_event_pattern_with_negated_agent() {
        let pattern = EventPattern {
            kind: Some(EventKind::FileWrite),
            agent: Some("admin".to_string()),
            target: None,
            task_id: None,
            negate_agent: true,
            target_not: Vec::new(),
        };

        // Non-admin agent matches (negated)
        let event = make_event(EventKind::FileWrite, "agent-1", "src/main.rs");
        assert!(pattern.matches_event(&event));

        // Admin agent does not match (negated)
        let event = make_event(EventKind::FileWrite, "admin", "src/main.rs");
        assert!(!pattern.matches_event(&event));
    }

    #[test]
    fn test_event_pattern_with_target_exclusion() {
        let pattern = EventPattern {
            kind: Some(EventKind::FileWrite),
            agent: None,
            target: None,
            task_id: None,
            negate_agent: false,
            target_not: vec![GlobPattern::new("docs/**"), GlobPattern::new("*.md")],
        };

        // Normal file matches
        let event = make_event(EventKind::FileWrite, "agent-1", "src/main.rs");
        assert!(pattern.matches_event(&event));

        // Excluded file does not match
        let event = make_event(EventKind::FileWrite, "agent-1", "docs/readme.md");
        assert!(!pattern.matches_event(&event));

        // Another excluded pattern
        let event = make_event(EventKind::FileWrite, "agent-1", "CHANGELOG.md");
        assert!(!pattern.matches_event(&event));
    }

    #[test]
    fn test_record_then_evaluate_full_cycle() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:1".to_string(),
            name: "File mutex".to_string(),
            priority: 5,
            enabled: true,
            rules: vec![BThreadRule::Mutex {
                scope: EventPattern::kind(EventKind::FileWrite),
                key: "file:{target}".to_string(),
                ttl_secs: Some(3600),
            }],
        });

        // Step 1: agent-1 records a FileWrite (acquires lock)
        let event1 = make_event(EventKind::FileWrite, "agent-1", "src/main.rs");
        coord.record(&event1).unwrap();
        assert!(coord.active_locks.contains_key("file:src/main.rs"));

        // Step 2: agent-2 tries to write same file -> BLOCKED
        let event2 = make_event(EventKind::FileWrite, "agent-2", "src/main.rs");
        assert!(coord.evaluate(&event2).is_blocked());

        // Step 3: agent-1 releases the lock
        coord.release("file:src/main.rs", "agent-1").unwrap();
        assert!(!coord.active_locks.contains_key("file:src/main.rs"));

        // Step 4: agent-2 can now write -> PROCEED
        assert!(coord.evaluate(&event2).is_proceed());
    }

    #[test]
    fn test_expired_lock_allows_access() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:1".to_string(),
            name: "File mutex".to_string(),
            priority: 5,
            enabled: true,
            rules: vec![BThreadRule::Mutex {
                scope: EventPattern::kind(EventKind::FileWrite),
                key: "file:{target}".to_string(),
                ttl_secs: Some(1),
            }],
        });

        // Insert lock that's already expired
        let old = Utc::now() - chrono::Duration::hours(1);
        coord.active_locks.insert(
            "file:src/main.rs".to_string(),
            ActiveLock {
                lock_key: "file:src/main.rs".to_string(),
                holder_agent: "agent-1".to_string(),
                task_id: None,
                acquired_at: old.to_rfc3339(),
                ttl_secs: 1,
            },
        );

        // Even without calling expire_locks, the evaluate should see it's expired
        let event = make_event(EventKind::FileWrite, "agent-2", "src/main.rs");
        assert!(coord.evaluate(&event).is_proceed());
    }

    #[test]
    fn test_decision_display() {
        let proceed = Decision::Proceed;
        assert_eq!(format!("{}", proceed), "PROCEED");

        let wait = Decision::Wait {
            reason: "needs tests".to_string(),
            thread_id: "w:1".to_string(),
        };
        assert!(format!("{}", wait).contains("WAIT"));
        assert!(format!("{}", wait).contains("needs tests"));

        let blocked = Decision::Blocked {
            reason: "mutex held".to_string(),
            thread_id: "w:2".to_string(),
        };
        assert!(format!("{}", blocked).contains("BLOCKED"));
        assert!(format!("{}", blocked).contains("mutex held"));
    }

    #[test]
    fn test_block_until() {
        let mut coord = Coordinator::new();
        coord.threads.push(BThread {
            id: "w:5".to_string(),
            name: "API review gate".to_string(),
            priority: 15,
            enabled: true,
            rules: vec![BThreadRule::BlockUntil {
                trigger: EventPattern::kind(EventKind::ApiChange),
                block: vec![EventPattern::kind(EventKind::Build)],
                until: EventPattern::kind(EventKind::Custom("ApiReviewApproved".to_string())),
                escalate: false,
                escalation_message: None,
            }],
        });

        // No trigger yet -> build proceeds
        let build = make_event(EventKind::Build, "agent-1", "");
        assert!(coord.evaluate(&build).is_proceed());

        // Trigger ApiChange
        coord.event_log.push(make_timestamped(
            make_event(EventKind::ApiChange, "agent-1", "src/api.rs"),
            &Utc::now().to_rfc3339(),
        ));

        // Build now blocked
        assert!(coord.evaluate(&build).is_blocked());

        // Resolve with ApiReviewApproved
        coord.event_log.push(make_timestamped(
            make_event(EventKind::Custom("ApiReviewApproved".to_string()), "reviewer", ""),
            &Utc::now().to_rfc3339(),
        ));

        // Build proceeds again
        assert!(coord.evaluate(&build).is_proceed());
    }
}
