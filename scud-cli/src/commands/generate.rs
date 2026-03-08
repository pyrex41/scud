//! Generate command - combines parse, expand, and check-deps into a single pipeline.

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::attractor::interviewer::{ConsoleInterviewer, Interviewer, Question};
use crate::commands::{ai, check_deps};
use crate::formats::{
    serialize_scg_pipeline, PipelineNodeAttrs, ScgEdgeAttrs, ScgParseResult, ScgPipeline,
};
use crate::llm::{LLMClient, Prompts};
use crate::models::{Phase, Priority, Task, TaskStatus};

/// Options for the task generation pipeline.
///
/// This struct configures the multi-phase task generation process:
/// 1. **Parse**: Convert a PRD document into initial tasks
/// 2. **Expand**: Break down complex tasks into subtasks
/// 3. **Check Dependencies**: Validate and fix task dependencies
///
/// # Example
///
/// ```no_run
/// use scud::commands::generate::{generate, GenerateOptions};
/// use std::path::PathBuf;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let options = GenerateOptions::new(
///         PathBuf::from("docs/prd.md"),
///         "my-feature".to_string(),
///     );
///
///     generate(options).await?;
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct GenerateOptions {
    /// Project root directory (None for current directory)
    pub project_root: Option<PathBuf>,
    /// Path to the PRD/spec document to parse
    pub file: PathBuf,
    /// Tag name for generated tasks
    pub tag: String,
    /// Number of tasks to generate (default: 10)
    pub num_tasks: u32,
    /// Skip task expansion phase
    pub no_expand: bool,
    /// Skip dependency validation phase
    pub no_check_deps: bool,
    /// Append tasks to existing tag instead of replacing
    pub append: bool,
    /// Skip loading guidance from .scud/guidance/
    pub no_guidance: bool,
    /// Task ID format: "sequential" (default) or "uuid"
    pub id_format: String,
    /// Model to use for AI operations (overrides config)
    pub model: Option<String>,
    /// Show what would be done without making changes
    pub dry_run: bool,
    /// Verbose output showing each phase's details
    pub verbose: bool,
}

impl GenerateOptions {
    /// Create new options with required fields and sensible defaults.
    ///
    /// # Arguments
    ///
    /// * `file` - Path to the PRD/spec document
    /// * `tag` - Tag name for the generated tasks
    pub fn new(file: PathBuf, tag: String) -> Self {
        Self {
            project_root: None,
            file,
            tag,
            num_tasks: 10,
            no_expand: false,
            no_check_deps: false,
            append: false,
            no_guidance: false,
            id_format: "sequential".to_string(),
            model: None,
            dry_run: false,
            verbose: false,
        }
    }
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self {
            project_root: None,
            file: PathBuf::new(),
            tag: String::new(),
            num_tasks: 10,
            no_expand: false,
            no_check_deps: false,
            append: false,
            no_guidance: false,
            id_format: "sequential".to_string(),
            model: None,
            dry_run: false,
            verbose: false,
        }
    }
}

/// Run the task generation pipeline with the given options.
///
/// This is the main entry point for programmatic task generation.
/// It orchestrates the parse → expand → check-deps pipeline.
///
/// # Example
///
/// ```no_run
/// use scud::commands::generate::{generate, GenerateOptions};
/// use std::path::PathBuf;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let mut options = GenerateOptions::new(
///         PathBuf::from("requirements.md"),
///         "api".to_string(),
///     );
///     options.num_tasks = 15;
///     options.verbose = true;
///
///     generate(options).await?;
///     Ok(())
/// }
/// ```
pub async fn generate(options: GenerateOptions) -> Result<()> {
    run(
        options.project_root,
        &options.file,
        &options.tag,
        options.num_tasks,
        options.no_expand,
        options.no_check_deps,
        options.append,
        options.no_guidance,
        &options.id_format,
        options.model.as_deref(),
        options.dry_run,
        options.verbose,
    )
    .await
}

/// Run the generate pipeline: parse PRD → expand tasks → validate dependencies
///
/// This is the internal implementation used by the CLI. For programmatic usage,
/// prefer the [`generate`] function with [`GenerateOptions`].
#[allow(clippy::too_many_arguments)]
pub async fn run(
    project_root: Option<PathBuf>,
    file: &Path,
    tag: &str,
    num_tasks: u32,
    no_expand: bool,
    no_check_deps: bool,
    append: bool,
    no_guidance: bool,
    id_format: &str,
    model: Option<&str>,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    println!("{}", "━".repeat(50).blue());
    println!(
        "{} {}",
        "Generate Pipeline".blue().bold(),
        format!("(tag: {})", tag).cyan()
    );
    println!("{}", "━".repeat(50).blue());
    println!();

    if dry_run {
        println!("{} Dry run mode - no changes will be made", "ℹ".blue());
        println!();
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 1: Parse PRD into tasks
    // ═══════════════════════════════════════════════════════════════════════
    println!("{} Parsing PRD into tasks...", "Phase 1:".yellow().bold());

    if dry_run {
        println!(
            "  {} Would parse {} into tag '{}'",
            "→".cyan(),
            file.display(),
            tag
        );
        println!(
            "  {} Would create ~{} tasks (append: {})",
            "→".cyan(),
            num_tasks,
            append
        );
    } else {
        ai::parse_prd::run(
            project_root.clone(),
            file,
            tag,
            num_tasks,
            append,
            no_guidance,
            id_format,
            model,
        )
        .await?;
    }

    if verbose {
        println!("  {} Parse phase completed", "✓".green());
    }
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 2: Expand complex tasks into subtasks
    // ═══════════════════════════════════════════════════════════════════════
    if no_expand {
        println!(
            "{} Skipping expansion {}",
            "Phase 2:".yellow().bold(),
            "(--no-expand)".dimmed()
        );
    } else {
        println!(
            "{} Expanding complex tasks into subtasks...",
            "Phase 2:".yellow().bold()
        );

        if dry_run {
            println!(
                "  {} Would expand tasks with complexity >= 5 in tag '{}'",
                "→".cyan(),
                tag
            );
        } else {
            ai::expand::run(
                project_root.clone(),
                None,      // task_id - expand all
                false,     // all_tags - only current tag
                Some(tag), // tag
                no_guidance,
                model,
            )
            .await?;
        }

        if verbose {
            println!("  {} Expand phase completed", "✓".green());
        }
    }
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 3: Validate dependencies
    // ═══════════════════════════════════════════════════════════════════════
    if no_check_deps {
        println!(
            "{} Skipping dependency validation {}",
            "Phase 3:".yellow().bold(),
            "(--no-check-deps)".dimmed()
        );
    } else {
        println!(
            "{} Validating task dependencies...",
            "Phase 3:".yellow().bold()
        );

        if dry_run {
            println!(
                "  {} Would validate dependencies in tag '{}' against PRD",
                "→".cyan(),
                tag
            );
            println!(
                "  {} Would auto-fix issues including agent type assignments",
                "→".cyan()
            );
        } else {
            println!(
                "  {} Validating tasks against PRD and auto-fixing coverage gaps...",
                "→".cyan()
            );
            // Run check-deps with PRD validation and auto-fix enabled
            // This validates against the PRD and auto-fixes issues including agent assignments
            let check_result = check_deps::run(
                project_root.clone(),
                Some(tag),  // tag
                false,      // all_tags
                Some(file), // prd_file - validate against PRD
                true,       // fix - auto-fix issues
                model,
            )
            .await;

            // Log but don't fail the pipeline on dep issues
            match check_result {
                Ok(_) => {
                    println!(
                        "  {} PRD validation passed",
                        "✓".green()
                    );
                }
                Err(e) => {
                    println!(
                        "  {} Dependency check encountered issues: {}",
                        "⚠".yellow(),
                        e
                    );
                    println!(
                        "  {} Run '{}' to see details",
                        "ℹ".blue(),
                        "scud check-deps".green()
                    );
                }
            }
        }

        if verbose {
            println!("  {} Check-deps phase completed", "✓".green());
        }
    }
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Summary
    // ═══════════════════════════════════════════════════════════════════════
    println!("{}", "━".repeat(50).green());
    println!("{}", "✅ Generate pipeline complete!".green().bold());
    println!("{}", "━".repeat(50).green());
    println!();

    if dry_run {
        println!("{}", "Dry run - no changes were made.".yellow());
        println!("Run without --dry-run to execute the pipeline.");
    } else {
        println!("{}", "Next steps:".blue());
        println!("  1. Review tasks: scud list --tag {}", tag);
        println!("  2. View execution waves: scud waves --tag {}", tag);
        println!("  3. Start working: scud next --tag {}", tag);
    }
    println!();

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Pipeline generation from PRD
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct ParsedPipeline {
    name: String,
    goal: String,
    model_stylesheet: Option<String>,
    nodes: Vec<ParsedNode>,
    edges: Vec<ParsedEdge>,
}

#[derive(Debug, Deserialize)]
struct ParsedNode {
    id: String,
    title: String,
    handler_type: String,
    #[serde(default)]
    max_retries: u32,
    #[serde(default)]
    goal_gate: bool,
    retry_target: Option<String>,
    timeout: Option<String>,
    prompt: Option<String>,
    tool_command: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ParsedEdge {
    from: String,
    to: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    condition: Option<String>,
    #[serde(default)]
    weight: i32,
}

/// Convert a ParsedPipeline into an ScgParseResult ready for serialization.
fn parsed_pipeline_to_scg(parsed: &ParsedPipeline, tag: &str) -> ScgParseResult {
    let mut phase = Phase::new(tag.to_string());

    let mut node_attrs = HashMap::new();

    for node in &parsed.nodes {
        // Map prompt to description, tool_command to details
        let description = node.prompt.clone().unwrap_or_default();
        let details = node.tool_command.clone();

        let complexity = match node.handler_type.as_str() {
            "start" | "exit" => 0,
            "wait.human" => 0,
            "tool" => 3,
            "codergen" => 5,
            _ => 0,
        };

        let priority = match node.handler_type.as_str() {
            "codergen" => Priority::High,
            _ => Priority::Medium,
        };

        let task = Task {
            id: node.id.clone(),
            title: node.title.clone(),
            description,
            status: TaskStatus::Pending,
            complexity,
            priority,
            dependencies: Vec::new(),
            parent_id: None,
            subtasks: Vec::new(),
            details,
            test_strategy: None,
            created_at: None,
            updated_at: None,
            assigned_to: None,
            agent_type: None,
        };
        phase.tasks.push(task);

        node_attrs.insert(
            node.id.clone(),
            PipelineNodeAttrs {
                handler_type: node.handler_type.clone(),
                max_retries: node.max_retries,
                retry_target: node.retry_target.clone(),
                goal_gate: node.goal_gate,
                timeout: node.timeout.clone(),
            },
        );
    }

    let edge_attrs: Vec<ScgEdgeAttrs> = parsed
        .edges
        .iter()
        .map(|e| ScgEdgeAttrs {
            from: e.from.clone(),
            to: e.to.clone(),
            label: e.label.clone().unwrap_or_default(),
            condition: e.condition.clone().unwrap_or_default(),
            weight: e.weight,
        })
        .collect();

    ScgParseResult {
        phase,
        pipeline: Some(ScgPipeline {
            goal: Some(parsed.goal.clone()),
            model_stylesheet: parsed.model_stylesheet.clone(),
            node_attrs,
            edge_attrs,
        }),
    }
}

/// Run interview questions and return (goal, shape, human_checkpoints, tool_steps, model_tier).
async fn run_interview(
    interviewer: &dyn Interviewer,
    prd_first_line: &str,
) -> Result<(String, String, String, String, String)> {
    // Q1: Goal
    let goal_answer = interviewer
        .ask(Question {
            text: format!(
                "What is the high-level goal for this pipeline? (default: {})",
                prd_first_line
            ),
            choices: vec![],
            default: None,
        })
        .await;
    let goal = if goal_answer.text.is_empty() {
        prd_first_line.to_string()
    } else {
        goal_answer.text
    };

    // Q2: Workflow shape
    let shape_answer = interviewer
        .ask(Question {
            text: "What workflow shape best describes this pipeline?".to_string(),
            choices: vec![
                "Linear (A->B->C->done)".to_string(),
                "Branching with human review gates".to_string(),
                "Iterative with test-fix loops".to_string(),
                "Custom (describe in PRD)".to_string(),
            ],
            default: Some(0),
        })
        .await;

    // Q3: Human checkpoints
    let human_answer = interviewer
        .ask(Question {
            text: "Include human review gates?".to_string(),
            choices: vec![
                "Yes, include human review gates".to_string(),
                "No, fully automated".to_string(),
            ],
            default: Some(0),
        })
        .await;

    // Q4: Tool steps
    let tool_answer = interviewer
        .ask(Question {
            text: "Any shell commands to include? (e.g., 'cargo test', 'npm run build') - leave empty for none".to_string(),
            choices: vec![],
            default: None,
        })
        .await;

    // Q5: Model tier
    let model_answer = interviewer
        .ask(Question {
            text: "Which model tier for LLM steps?".to_string(),
            choices: vec![
                "Fast (Haiku - cheap, quick)".to_string(),
                "Balanced (Sonnet - recommended)".to_string(),
                "Powerful (Opus - best quality)".to_string(),
            ],
            default: Some(1),
        })
        .await;

    Ok((
        goal,
        shape_answer.text,
        human_answer.text,
        tool_answer.text,
        model_answer.text,
    ))
}

/// Generate an Attractor pipeline SCG from a PRD document.
#[allow(clippy::too_many_arguments)]
pub async fn run_pipeline(
    project_root: Option<PathBuf>,
    file: &Path,
    tag: &str,
    model: Option<&str>,
    output: Option<PathBuf>,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    println!("{}", "━".repeat(50).blue());
    println!(
        "{} {}",
        "Generate Attractor Pipeline".blue().bold(),
        format!("(tag: {})", tag).cyan()
    );
    println!("{}", "━".repeat(50).blue());
    println!();

    // Read PRD
    let prd_content = std::fs::read_to_string(file)
        .with_context(|| format!("reading PRD: {}", file.display()))?;

    let prd_first_line = prd_content
        .lines()
        .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .or_else(|| {
            prd_content
                .lines()
                .find(|l| !l.trim().is_empty())
                .map(|l| l.trim_start_matches('#').trim())
        })
        .unwrap_or("Build something great");

    // Interview
    println!("{} Interview", "Phase 1:".yellow().bold());
    let interviewer = ConsoleInterviewer;
    let (goal, shape, human_checkpoints, tool_steps, model_tier) =
        run_interview(&interviewer, prd_first_line).await?;

    if verbose {
        println!();
        println!("  {} Goal: {}", "→".cyan(), goal);
        println!("  {} Shape: {}", "→".cyan(), shape);
        println!("  {} Human gates: {}", "→".cyan(), human_checkpoints);
        println!(
            "  {} Tools: {}",
            "→".cyan(),
            if tool_steps.is_empty() {
                "(none)"
            } else {
                &tool_steps
            }
        );
        println!("  {} Model: {}", "→".cyan(), model_tier);
    }
    println!();

    if dry_run {
        println!(
            "{} Would generate pipeline with LLM...",
            "Phase 2:".yellow().bold()
        );
        println!(
            "  {} Would write to: {}",
            "→".cyan(),
            output
                .as_deref()
                .unwrap_or_else(|| Path::new(".scud/tasks/tasks.scg"))
                .display()
        );
        println!();
        println!("{}", "Dry run - no changes were made.".yellow());
        return Ok(());
    }

    // LLM generation
    println!(
        "{} Generating pipeline via LLM...",
        "Phase 2:".yellow().bold()
    );

    let prompt = Prompts::generate_pipeline(
        &prd_content,
        &goal,
        &shape,
        &human_checkpoints,
        &tool_steps,
        &model_tier,
    );

    let client = if let Some(ref root) = project_root {
        LLMClient::new_with_project_root(root.clone())?
    } else {
        LLMClient::new()?
    };
    let parsed: ParsedPipeline = client.complete_json_smart(&prompt, model).await?;

    if verbose {
        println!(
            "  {} Generated {} nodes, {} edges",
            "✓".green(),
            parsed.nodes.len(),
            parsed.edges.len()
        );
    }
    println!();

    // Convert to SCG
    println!("{} Converting to SCG format...", "Phase 3:".yellow().bold());

    let result = parsed_pipeline_to_scg(&parsed, tag);
    let scg_output = serialize_scg_pipeline(&result);

    // Determine output path
    let output_path = output.unwrap_or_else(|| {
        let root = project_root.unwrap_or_else(|| PathBuf::from("."));
        root.join(".scud/tasks/tasks.scg")
    });

    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directory: {}", parent.display()))?;
    }

    std::fs::write(&output_path, &scg_output)
        .with_context(|| format!("writing pipeline: {}", output_path.display()))?;

    if verbose {
        println!("  {} Written to {}", "✓".green(), output_path.display());
    }
    println!();

    // Summary
    println!("{}", "━".repeat(50).green());
    println!("{}", "Pipeline generated successfully!".green().bold());
    println!("{}", "━".repeat(50).green());
    println!();
    println!(
        "  {} {} nodes, {} edges",
        "→".cyan(),
        result.phase.tasks.len(),
        result
            .pipeline
            .as_ref()
            .map(|p| p.edge_attrs.len())
            .unwrap_or(0)
    );
    println!("  {} Output: {}", "→".cyan(), output_path.display());
    println!();
    println!("{}", "Next steps:".blue());
    println!(
        "  1. Validate: scud attractor validate {}",
        output_path.display()
    );
    println!("  2. Run: scud attractor run {}", output_path.display());
    println!();

    Ok(())
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;
    use crate::formats::parse_scg_result;

    fn sample_parsed_pipeline() -> ParsedPipeline {
        ParsedPipeline {
            name: "test-pipe".to_string(),
            goal: "Build something".to_string(),
            model_stylesheet: Some(
                r#"* { model: "claude-3-haiku"; reasoning_effort: "medium" }"#.to_string(),
            ),
            nodes: vec![
                ParsedNode {
                    id: "start".to_string(),
                    title: "Start".to_string(),
                    handler_type: "start".to_string(),
                    max_retries: 0,
                    goal_gate: false,
                    retry_target: None,
                    timeout: None,
                    prompt: None,
                    tool_command: None,
                },
                ParsedNode {
                    id: "design".to_string(),
                    title: "Design API".to_string(),
                    handler_type: "codergen".to_string(),
                    max_retries: 3,
                    goal_gate: false,
                    retry_target: None,
                    timeout: None,
                    prompt: Some("Design the REST API schema".to_string()),
                    tool_command: None,
                },
                ParsedNode {
                    id: "test".to_string(),
                    title: "Run Tests".to_string(),
                    handler_type: "tool".to_string(),
                    max_retries: 0,
                    goal_gate: false,
                    retry_target: None,
                    timeout: None,
                    prompt: None,
                    tool_command: Some("cargo test".to_string()),
                },
                ParsedNode {
                    id: "finish".to_string(),
                    title: "Done".to_string(),
                    handler_type: "exit".to_string(),
                    max_retries: 0,
                    goal_gate: true,
                    retry_target: Some("design".to_string()),
                    timeout: None,
                    prompt: None,
                    tool_command: None,
                },
            ],
            edges: vec![
                ParsedEdge {
                    from: "start".to_string(),
                    to: "design".to_string(),
                    label: None,
                    condition: None,
                    weight: 0,
                },
                ParsedEdge {
                    from: "design".to_string(),
                    to: "test".to_string(),
                    label: None,
                    condition: None,
                    weight: 0,
                },
                ParsedEdge {
                    from: "test".to_string(),
                    to: "finish".to_string(),
                    label: None,
                    condition: Some("outcome=success".to_string()),
                    weight: 0,
                },
                ParsedEdge {
                    from: "test".to_string(),
                    to: "design".to_string(),
                    label: Some("Fix".to_string()),
                    condition: Some("outcome=failure".to_string()),
                    weight: 0,
                },
            ],
        }
    }

    #[test]
    fn test_parsed_pipeline_to_scg_conversion() {
        let parsed = sample_parsed_pipeline();
        let result = parsed_pipeline_to_scg(&parsed, "test-pipe");

        assert_eq!(result.phase.name, "test-pipe");
        assert_eq!(result.phase.tasks.len(), 4);
        assert!(result.pipeline.is_some());

        let pipeline = result.pipeline.as_ref().unwrap();
        assert_eq!(pipeline.goal.as_deref(), Some("Build something"));
        assert_eq!(pipeline.node_attrs.len(), 4);
        assert_eq!(pipeline.edge_attrs.len(), 4);

        // Check node attrs
        let design_attrs = &pipeline.node_attrs["design"];
        assert_eq!(design_attrs.handler_type, "codergen");
        assert_eq!(design_attrs.max_retries, 3);

        let finish_attrs = &pipeline.node_attrs["finish"];
        assert_eq!(finish_attrs.handler_type, "exit");
        assert!(finish_attrs.goal_gate);
        assert_eq!(finish_attrs.retry_target.as_deref(), Some("design"));

        // Check task fields
        let design_task = result
            .phase
            .tasks
            .iter()
            .find(|t| t.id == "design")
            .unwrap();
        assert_eq!(design_task.description, "Design the REST API schema");
        assert_eq!(design_task.complexity, 5); // codergen

        let test_task = result.phase.tasks.iter().find(|t| t.id == "test").unwrap();
        assert_eq!(test_task.details.as_deref(), Some("cargo test"));
    }

    #[test]
    fn test_pipeline_round_trip() {
        let parsed = sample_parsed_pipeline();
        let result = parsed_pipeline_to_scg(&parsed, "test-pipe");
        let serialized = serialize_scg_pipeline(&result);

        // Parse it back
        let reparsed = parse_scg_result(&serialized).expect("should parse serialized pipeline");

        assert_eq!(reparsed.phase.name, "test-pipe");
        assert_eq!(reparsed.phase.tasks.len(), 4);
        assert!(reparsed.pipeline.is_some());

        let pipeline = reparsed.pipeline.as_ref().unwrap();
        assert_eq!(pipeline.goal.as_deref(), Some("Build something"));
        assert_eq!(pipeline.node_attrs.len(), 4);
        assert_eq!(pipeline.edge_attrs.len(), 4);

        // Verify key attributes survived round-trip
        let design_attrs = &pipeline.node_attrs["design"];
        assert_eq!(design_attrs.handler_type, "codergen");
        assert_eq!(design_attrs.max_retries, 3);

        let finish_attrs = &pipeline.node_attrs["finish"];
        assert!(finish_attrs.goal_gate);
        assert_eq!(finish_attrs.retry_target.as_deref(), Some("design"));
    }

    #[test]
    fn test_prompt_builder_contains_key_markers() {
        let prompt = Prompts::generate_pipeline(
            "Build a REST API for users",
            "Build user management API",
            "Iterative with test-fix loops",
            "Yes, include human review gates",
            "cargo test",
            "Balanced (Sonnet - recommended)",
        );

        assert!(prompt.contains("Build a REST API for users")); // PRD content
        assert!(prompt.contains("Build user management API")); // goal
        assert!(prompt.contains("Iterative with test-fix loops")); // shape
        assert!(prompt.contains("cargo test")); // tool steps
        assert!(prompt.contains("handler_type")); // format teaching
        assert!(prompt.contains("codergen")); // handler type example
        assert!(prompt.contains("wait.human")); // handler type
        assert!(prompt.contains("model_stylesheet")); // stylesheet
    }
}
