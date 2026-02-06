use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use scud_core::weave::{BThread, BThreadRule, EventKind, EventPattern};

use super::check::{core_storage, resolve_tag};

struct TemplateEntry {
    name: &'static str,
    description: &'static str,
    thread_name: &'static str,
    priority: u32,
    rule: fn() -> BThreadRule,
}

fn templates() -> Vec<TemplateEntry> {
    vec![
        TemplateEntry {
            name: "commit-gate",
            description: "Require tests+lint before commit",
            thread_name: "Commit gate",
            priority: 10,
            rule: || BThreadRule::Require {
                trigger: EventPattern::kind(EventKind::Commit),
                prerequisite: EventPattern::kind(EventKind::TestPass),
                reset: Some(EventPattern::kind(EventKind::FileWrite)),
            },
        },
        TemplateEntry {
            name: "file-mutex",
            description: "Per-file mutual exclusion",
            thread_name: "File mutex",
            priority: 5,
            rule: || BThreadRule::Mutex {
                scope: EventPattern::kind(EventKind::FileWrite),
                key: "file:{target}".to_string(),
                ttl_secs: Some(3600),
            },
        },
        TemplateEntry {
            name: "schema-singleton",
            description: "Single-writer for migrations/schema",
            thread_name: "Schema singleton",
            priority: 1,
            rule: || BThreadRule::Mutex {
                scope: EventPattern::kind(EventKind::SchemaChange),
                key: "schema-global".to_string(),
                ttl_secs: Some(7200),
            },
        },
        TemplateEntry {
            name: "api-review-gate",
            description: "Block builds after API changes until review",
            thread_name: "API review gate",
            priority: 15,
            rule: || BThreadRule::BlockUntil {
                trigger: EventPattern::kind(EventKind::ApiChange),
                block: vec![EventPattern::kind(EventKind::Build)],
                until: EventPattern::kind(EventKind::Custom("ApiReviewApproved".to_string())),
                escalate: false,
                escalation_message: None,
            },
        },
        TemplateEntry {
            name: "dep-change-gate",
            description: "Block after dependency changes until lockfile verified",
            thread_name: "Dependency change gate",
            priority: 12,
            rule: || BThreadRule::BlockUntil {
                trigger: EventPattern::kind(EventKind::DependencyAdd),
                block: vec![EventPattern::kind(EventKind::Build)],
                until: EventPattern::kind(EventKind::Custom("LockfileVerified".to_string())),
                escalate: false,
                escalation_message: None,
            },
        },
        TemplateEntry {
            name: "rate-limit-commits",
            description: "Prevent commit storms (max 5 per 300s)",
            thread_name: "Commit rate limit",
            priority: 20,
            rule: || BThreadRule::RateLimit {
                scope: EventPattern::kind(EventKind::Commit),
                max: 5,
                window_secs: 300,
            },
        },
        TemplateEntry {
            name: "no-self-kill",
            description: "Block pkill, kill -9, rm -rf, shutdown",
            thread_name: "No dangerous commands",
            priority: 1,
            rule: || BThreadRule::BlockAlways {
                scope: EventPattern::kind(EventKind::DangerousCommand),
            },
        },
        TemplateEntry {
            name: "test-timeout",
            description: "Kill test runs exceeding 5 minutes",
            thread_name: "Test timeout",
            priority: 25,
            rule: || BThreadRule::Timeout {
                scope: EventPattern::kind(EventKind::TestRun),
                max_duration_secs: 300,
                action: scud_core::weave::bthread::TimeoutAction::Kill,
            },
        },
        TemplateEntry {
            name: "build-serializer",
            description: "Serialize builds (only one at a time)",
            thread_name: "Build serializer",
            priority: 8,
            rule: || BThreadRule::Mutex {
                scope: EventPattern::kind(EventKind::Build),
                key: "build-global".to_string(),
                ttl_secs: Some(600),
            },
        },
    ]
}

pub fn run_list() -> Result<()> {
    println!("{}\n", "Available Templates:".blue().bold());

    for t in templates() {
        println!("  {:<22} {}", t.name.cyan(), t.description);
    }

    println!(
        "\nApply with: {}",
        "scud weave template apply <name>".dimmed()
    );
    Ok(())
}

pub fn run_apply(project_root: Option<PathBuf>, name: &str) -> Result<()> {
    let entry = templates()
        .into_iter()
        .find(|t| t.name == name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown template '{}'. Run 'scud weave template list' to see available templates.",
                name
            )
        })?;

    let storage = core_storage(project_root);
    let phase_tag = resolve_tag(&storage)?;
    let mut phase = storage.load_group(&phase_tag)?;

    // Generate unique ID
    let next_num = phase.weave_threads.len() + 1;
    let id = format!("w:{}", next_num);

    // Check if a thread with this name already exists
    if phase
        .weave_threads
        .iter()
        .any(|t| t.name == entry.thread_name)
    {
        anyhow::bail!(
            "A b-thread named \"{}\" already exists. Remove it first or use a different template.",
            entry.thread_name
        );
    }

    let bthread = BThread {
        id: id.clone(),
        name: entry.thread_name.to_string(),
        priority: entry.priority,
        enabled: false, // Templates start disabled
        rules: vec![(entry.rule)()],
    };

    phase.weave_threads.push(bthread);

    // Ensure weave directory exists
    let weave_dir = storage.scud_dir().join("weave");
    std::fs::create_dir_all(&weave_dir)?;

    storage.update_group(&phase_tag, &phase)?;

    println!(
        "Applied \"{}\" -> @weave as {} (disabled)",
        name, id
    );
    println!("Enable with: scud weave enable {}", id);
    Ok(())
}
