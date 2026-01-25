use scud_eval::{metrics, runner, storage, tasksets};

// =============================================================================
// Taskset Loading Tests
// =============================================================================

#[test]
fn test_builtin_tasksets_exist() {
    let sets = tasksets::builtin_tasksets();
    assert!(!sets.is_empty(), "builtin_tasksets should not be empty");

    // Check expected tasksets exist
    let names: Vec<_> = sets.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"eval-trivial"),
        "Should contain eval-trivial taskset"
    );
    assert!(
        names.contains(&"eval-moderate"),
        "Should contain eval-moderate taskset"
    );
    assert!(
        names.contains(&"eval-complex"),
        "Should contain eval-complex taskset"
    );
    assert!(
        names.contains(&"eval-real-scud"),
        "Should contain eval-real-scud taskset"
    );
}

#[test]
fn test_builtin_tasksets_have_valid_structure() {
    let sets = tasksets::builtin_tasksets();

    for taskset in sets {
        // Each taskset should have a non-empty name
        assert!(!taskset.name.is_empty(), "Taskset name should not be empty");

        // Each taskset should have a description
        assert!(
            !taskset.description.is_empty(),
            "Taskset {} should have a description",
            taskset.name
        );

        // Each taskset should have SCG content
        assert!(
            !taskset.scg_content.is_empty(),
            "Taskset {} should have SCG content",
            taskset.name
        );

        // SCG content should start with a comment
        assert!(
            taskset.scg_content.starts_with('#'),
            "Taskset {} SCG should start with a comment",
            taskset.name
        );

        // Each taskset should have expected files
        assert!(
            !taskset.expected_files.is_empty(),
            "Taskset {} should have expected files",
            taskset.name
        );
    }
}

#[test]
fn test_trivial_taskset_properties() {
    let sets = tasksets::builtin_tasksets();
    let trivial = sets
        .iter()
        .find(|s| s.name == "eval-trivial")
        .expect("eval-trivial taskset should exist");

    // Check that trivial taskset has expected files
    assert!(trivial.expected_files.contains(&"hello.py".to_string()));
    assert!(trivial.expected_files.contains(&"goodbye.py".to_string()));
    assert!(trivial.expected_files.contains(&"utils.py".to_string()));
    assert!(trivial.expected_files.contains(&"constants.py".to_string()));
    assert!(trivial.expected_files.contains(&"README.md".to_string()));

    // Check description mentions 5 independent tasks
    assert!(
        trivial.description.contains("5"),
        "Trivial taskset should mention 5 tasks"
    );
}

#[test]
fn test_moderate_taskset_has_dependencies() {
    let sets = tasksets::builtin_tasksets();
    let moderate = sets
        .iter()
        .find(|s| s.name == "eval-moderate")
        .expect("eval-moderate taskset should exist");

    // The SCG content should have edges (dependencies)
    assert!(
        moderate.scg_content.contains("@edges"),
        "Moderate taskset should have @edges section"
    );
    assert!(
        moderate.scg_content.contains("->"),
        "Moderate taskset should have dependency arrows"
    );
}

// =============================================================================
// Workspace Setup Tests
// =============================================================================

#[test]
fn test_workspace_setup() {
    let sets = tasksets::builtin_tasksets();
    let trivial = sets
        .iter()
        .find(|s| s.name == "eval-trivial")
        .expect("eval-trivial taskset should exist");

    let workspace = runner::setup_eval_workspace(trivial, "test-run-123")
        .expect("Workspace setup should succeed");

    // Verify workspace path exists
    assert!(workspace.path.exists(), "Workspace path should exist");

    // Verify .scud directory structure
    assert!(
        workspace.path.join(".scud").exists(),
        ".scud directory should exist"
    );
    assert!(
        workspace.path.join(".scud/tasks").exists(),
        ".scud/tasks directory should exist"
    );
    assert!(
        workspace.path.join(".scud/tasks/tasks.scg").exists(),
        "tasks.scg file should exist"
    );
    assert!(
        workspace.path.join(".scud/active-tag").exists(),
        "active-tag file should exist"
    );

    // Verify active tag content
    let active_tag =
        std::fs::read_to_string(workspace.path.join(".scud/active-tag")).expect("Should read active-tag");
    assert_eq!(
        active_tag.trim(),
        "eval-trivial",
        "Active tag should match taskset name"
    );

    // Verify SCG content was written
    let scg_content = std::fs::read_to_string(workspace.path.join(".scud/tasks/tasks.scg"))
        .expect("Should read tasks.scg");
    assert!(
        scg_content.contains("eval-trivial"),
        "SCG should contain taskset name"
    );

    // Verify it's a git repo
    assert!(
        workspace.path.join(".git").exists(),
        "Workspace should be a git repository"
    );

    // Verify workspace properties
    assert_eq!(workspace.taskset_name, "eval-trivial");
    assert_eq!(workspace.run_id, "test-run-123");

    // Cleanup: The workspace is in ~/.scud-eval/runs/ so we should clean it up
    let _ = std::fs::remove_dir_all(storage::run_dir("test-run-123"));
}

#[test]
fn test_workspace_setup_with_different_taskset() {
    let sets = tasksets::builtin_tasksets();
    let moderate = sets
        .iter()
        .find(|s| s.name == "eval-moderate")
        .expect("eval-moderate taskset should exist");

    let run_id = "test-run-moderate-456";
    let workspace =
        runner::setup_eval_workspace(moderate, run_id).expect("Workspace setup should succeed");

    // Verify workspace properties
    assert_eq!(workspace.taskset_name, "eval-moderate");
    assert_eq!(workspace.run_id, run_id);

    // Verify active tag matches
    let active_tag =
        std::fs::read_to_string(workspace.path.join(".scud/active-tag")).expect("Should read active-tag");
    assert_eq!(active_tag.trim(), "eval-moderate");

    // Cleanup
    let _ = std::fs::remove_dir_all(storage::run_dir(run_id));
}

// =============================================================================
// Metrics Serialization Tests
// =============================================================================

#[test]
fn test_eval_run_metrics_roundtrip() {
    use chrono::Utc;

    // Create test metrics
    let metrics = metrics::EvalRunMetrics {
        run_id: "test-roundtrip-run".to_string(),
        mode: metrics::ExecutionMode::Ralph,
        taskset_name: "eval-trivial".to_string(),
        harness: "test-harness".to_string(),
        model: Some("claude-opus-4-5-20251101".to_string()),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        total_duration_secs: Some(120.5),
        total_tasks: 5,
        tasks_succeeded: 4,
        tasks_failed: 1,
        first_pass_success_rate: 0.8,
        total_repair_attempts: 2,
        total_lines_added: 150,
        total_lines_removed: 20,
        total_files_changed: 5,
        total_tokens_input: Some(10000),
        total_tokens_output: Some(5000),
        total_estimated_cost_usd: Some(0.15),
        task_metrics: vec![],
        validation_commands: vec![],
    };

    // Serialize to JSON
    let json =
        serde_json::to_string_pretty(&metrics).expect("Serialization should succeed");

    // Verify JSON contains expected fields
    assert!(json.contains("test-roundtrip-run"));
    assert!(json.contains("eval-trivial"));
    assert!(json.contains("Ralph"));
    assert!(json.contains("claude-opus-4-5-20251101"));

    // Deserialize back
    let deserialized: metrics::EvalRunMetrics =
        serde_json::from_str(&json).expect("Deserialization should succeed");

    // Verify roundtrip preserved data
    assert_eq!(deserialized.run_id, metrics.run_id);
    assert_eq!(deserialized.mode, metrics.mode);
    assert_eq!(deserialized.taskset_name, metrics.taskset_name);
    assert_eq!(deserialized.harness, metrics.harness);
    assert_eq!(deserialized.model, metrics.model);
    assert_eq!(deserialized.total_tasks, metrics.total_tasks);
    assert_eq!(deserialized.tasks_succeeded, metrics.tasks_succeeded);
    assert_eq!(deserialized.tasks_failed, metrics.tasks_failed);
    assert_eq!(
        deserialized.first_pass_success_rate,
        metrics.first_pass_success_rate
    );
    assert_eq!(deserialized.total_tokens_input, metrics.total_tokens_input);
}

#[test]
fn test_task_metrics_roundtrip() {
    use chrono::Utc;

    let task = metrics::TaskMetrics {
        task_id: "task-1".to_string(),
        task_title: "Create hello.py".to_string(),
        complexity: 1,
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        duration_secs: Some(45.2),
        success: true,
        first_pass_success: true,
        repair_attempts: 0,
        lines_added: Some(10),
        lines_removed: Some(0),
        files_changed: Some(1),
        tokens_input: Some(500),
        tokens_output: Some(200),
        estimated_cost_usd: Some(0.01),
    };

    let json = serde_json::to_string(&task).expect("Serialization should succeed");
    let deserialized: metrics::TaskMetrics =
        serde_json::from_str(&json).expect("Deserialization should succeed");

    assert_eq!(deserialized.task_id, task.task_id);
    assert_eq!(deserialized.task_title, task.task_title);
    assert_eq!(deserialized.complexity, task.complexity);
    assert_eq!(deserialized.success, task.success);
    assert_eq!(deserialized.first_pass_success, task.first_pass_success);
    assert_eq!(deserialized.repair_attempts, task.repair_attempts);
}

#[test]
fn test_execution_mode_variants() {
    // Test Swarm mode
    let swarm = metrics::ExecutionMode::Swarm { round_size: 4 };
    assert_eq!(swarm.name(), "swarm-4");

    let swarm_json = serde_json::to_string(&swarm).expect("Serialization should succeed");
    let swarm_deser: metrics::ExecutionMode =
        serde_json::from_str(&swarm_json).expect("Deserialization should succeed");
    assert_eq!(swarm, swarm_deser);

    // Test Ralph mode
    let ralph = metrics::ExecutionMode::Ralph;
    assert_eq!(ralph.name(), "ralph");

    let ralph_json = serde_json::to_string(&ralph).expect("Serialization should succeed");
    let ralph_deser: metrics::ExecutionMode =
        serde_json::from_str(&ralph_json).expect("Deserialization should succeed");
    assert_eq!(ralph, ralph_deser);

    // Test ClaudeDirect mode
    let direct = metrics::ExecutionMode::ClaudeDirect;
    assert_eq!(direct.name(), "claude-direct");

    let direct_json = serde_json::to_string(&direct).expect("Serialization should succeed");
    let direct_deser: metrics::ExecutionMode =
        serde_json::from_str(&direct_json).expect("Deserialization should succeed");
    assert_eq!(direct, direct_deser);
}

#[test]
fn test_validation_metrics_roundtrip() {
    let validation = metrics::ValidationMetrics {
        command: "pytest tests/".to_string(),
        passed: true,
        duration_secs: 5.5,
        run_count: 3,
    };

    let json = serde_json::to_string(&validation).expect("Serialization should succeed");
    let deserialized: metrics::ValidationMetrics =
        serde_json::from_str(&json).expect("Deserialization should succeed");

    assert_eq!(deserialized.command, validation.command);
    assert_eq!(deserialized.passed, validation.passed);
    assert_eq!(deserialized.duration_secs, validation.duration_secs);
    assert_eq!(deserialized.run_count, validation.run_count);
}

// =============================================================================
// Storage Operations Tests
// =============================================================================

#[test]
fn test_save_and_load_run() {
    use chrono::Utc;

    let run_id = "test-storage-run-789";

    // Create test metrics
    let metrics = metrics::EvalRunMetrics {
        run_id: run_id.to_string(),
        mode: metrics::ExecutionMode::Swarm { round_size: 3 },
        taskset_name: "eval-trivial".to_string(),
        harness: "test-harness".to_string(),
        model: Some("claude-sonnet-4-20250514".to_string()),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        total_duration_secs: Some(60.0),
        total_tasks: 5,
        tasks_succeeded: 5,
        tasks_failed: 0,
        first_pass_success_rate: 1.0,
        total_repair_attempts: 0,
        total_lines_added: 100,
        total_lines_removed: 0,
        total_files_changed: 5,
        total_tokens_input: Some(8000),
        total_tokens_output: Some(4000),
        total_estimated_cost_usd: Some(0.10),
        task_metrics: vec![],
        validation_commands: vec![],
    };

    // Save the run
    let saved_path = storage::save_run(&metrics).expect("save_run should succeed");
    assert!(saved_path.exists(), "Saved metrics file should exist");
    assert!(
        saved_path.ends_with("metrics.json"),
        "File should be named metrics.json"
    );

    // Load the run back
    let loaded = storage::load_run(run_id).expect("load_run should succeed");

    // Verify loaded data matches saved data
    assert_eq!(loaded.run_id, metrics.run_id);
    assert_eq!(loaded.mode, metrics.mode);
    assert_eq!(loaded.taskset_name, metrics.taskset_name);
    assert_eq!(loaded.total_tasks, metrics.total_tasks);
    assert_eq!(loaded.tasks_succeeded, metrics.tasks_succeeded);
    assert_eq!(loaded.first_pass_success_rate, metrics.first_pass_success_rate);

    // Cleanup
    let _ = std::fs::remove_dir_all(storage::run_dir(run_id));
}

#[test]
fn test_storage_paths() {
    // Verify storage paths are reasonable
    let home = storage::eval_home();
    assert!(
        home.to_string_lossy().contains(".scud-eval"),
        "Eval home should contain .scud-eval"
    );

    let runs = storage::runs_dir();
    assert!(
        runs.to_string_lossy().contains("runs"),
        "Runs dir should contain 'runs'"
    );

    let tasksets = storage::tasksets_dir();
    assert!(
        tasksets.to_string_lossy().contains("tasksets"),
        "Tasksets dir should contain 'tasksets'"
    );

    let specific_run = storage::run_dir("my-run-id");
    assert!(
        specific_run.to_string_lossy().contains("my-run-id"),
        "Run dir should contain run ID"
    );
}

#[test]
fn test_list_runs() {
    use chrono::Utc;

    // Create a couple of test runs
    let run_ids = ["test-list-run-a", "test-list-run-b"];

    for run_id in &run_ids {
        let metrics = metrics::EvalRunMetrics {
            run_id: run_id.to_string(),
            mode: metrics::ExecutionMode::Ralph,
            taskset_name: "eval-trivial".to_string(),
            harness: "test".to_string(),
            model: None,
            started_at: Utc::now(),
            completed_at: None,
            total_duration_secs: None,
            total_tasks: 1,
            tasks_succeeded: 0,
            tasks_failed: 0,
            first_pass_success_rate: 0.0,
            total_repair_attempts: 0,
            total_lines_added: 0,
            total_lines_removed: 0,
            total_files_changed: 0,
            total_tokens_input: None,
            total_tokens_output: None,
            total_estimated_cost_usd: None,
            task_metrics: vec![],
            validation_commands: vec![],
        };
        storage::save_run(&metrics).expect("save_run should succeed");
    }

    // List runs
    let runs = storage::list_runs().expect("list_runs should succeed");

    // Verify our test runs are in the list
    for run_id in &run_ids {
        assert!(
            runs.contains(&run_id.to_string()),
            "Run list should contain {}",
            run_id
        );
    }

    // Cleanup
    for run_id in &run_ids {
        let _ = std::fs::remove_dir_all(storage::run_dir(run_id));
    }
}

#[test]
fn test_load_nonexistent_run_fails() {
    let result = storage::load_run("this-run-does-not-exist-12345");
    assert!(
        result.is_err(),
        "Loading non-existent run should return an error"
    );
}

// =============================================================================
// Taskset Installation Tests
// =============================================================================

#[test]
fn test_install_taskset() {
    let sets = tasksets::builtin_tasksets();
    let trivial = sets
        .iter()
        .find(|s| s.name == "eval-trivial")
        .expect("eval-trivial taskset should exist");

    let installed_path =
        tasksets::install_taskset(trivial).expect("install_taskset should succeed");

    // Verify installation path
    assert!(installed_path.exists(), "Installed path should exist");
    assert!(
        installed_path.join("tasks.scg").exists(),
        "tasks.scg should exist"
    );
    assert!(
        installed_path.join("taskset.json").exists(),
        "taskset.json should exist"
    );

    // Verify tasks.scg content
    let scg_content = std::fs::read_to_string(installed_path.join("tasks.scg"))
        .expect("Should read tasks.scg");
    assert!(scg_content.contains("eval-trivial"));

    // Verify taskset.json content
    let taskset_json = std::fs::read_to_string(installed_path.join("taskset.json"))
        .expect("Should read taskset.json");
    let parsed: tasksets::TaskSet =
        serde_json::from_str(&taskset_json).expect("Should parse taskset.json");
    assert_eq!(parsed.name, "eval-trivial");

    // Cleanup
    let _ = std::fs::remove_dir_all(installed_path);
}

// =============================================================================
// Metrics with Task Details Tests
// =============================================================================

#[test]
fn test_eval_run_metrics_with_task_metrics() {
    use chrono::Utc;

    let task1 = metrics::TaskMetrics {
        task_id: "1".to_string(),
        task_title: "Create hello.py".to_string(),
        complexity: 1,
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        duration_secs: Some(30.0),
        success: true,
        first_pass_success: true,
        repair_attempts: 0,
        lines_added: Some(5),
        lines_removed: Some(0),
        files_changed: Some(1),
        tokens_input: Some(300),
        tokens_output: Some(100),
        estimated_cost_usd: Some(0.005),
    };

    let task2 = metrics::TaskMetrics {
        task_id: "2".to_string(),
        task_title: "Create goodbye.py".to_string(),
        complexity: 1,
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        duration_secs: Some(25.0),
        success: true,
        first_pass_success: false,
        repair_attempts: 1,
        lines_added: Some(5),
        lines_removed: Some(0),
        files_changed: Some(1),
        tokens_input: Some(400),
        tokens_output: Some(150),
        estimated_cost_usd: Some(0.007),
    };

    let run_metrics = metrics::EvalRunMetrics {
        run_id: "test-with-tasks".to_string(),
        mode: metrics::ExecutionMode::Swarm { round_size: 2 },
        taskset_name: "eval-trivial".to_string(),
        harness: "test".to_string(),
        model: None,
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
        total_duration_secs: Some(55.0),
        total_tasks: 2,
        tasks_succeeded: 2,
        tasks_failed: 0,
        first_pass_success_rate: 0.5,
        total_repair_attempts: 1,
        total_lines_added: 10,
        total_lines_removed: 0,
        total_files_changed: 2,
        total_tokens_input: Some(700),
        total_tokens_output: Some(250),
        total_estimated_cost_usd: Some(0.012),
        task_metrics: vec![task1, task2],
        validation_commands: vec![metrics::ValidationMetrics {
            command: "python -c 'import hello'".to_string(),
            passed: true,
            duration_secs: 0.5,
            run_count: 1,
        }],
    };

    // Serialize and deserialize
    let json = serde_json::to_string_pretty(&run_metrics).expect("Serialization should succeed");
    let loaded: metrics::EvalRunMetrics =
        serde_json::from_str(&json).expect("Deserialization should succeed");

    // Verify task metrics were preserved
    assert_eq!(loaded.task_metrics.len(), 2);
    assert_eq!(loaded.task_metrics[0].task_id, "1");
    assert_eq!(loaded.task_metrics[1].task_id, "2");
    assert!(loaded.task_metrics[0].first_pass_success);
    assert!(!loaded.task_metrics[1].first_pass_success);

    // Verify validation commands were preserved
    assert_eq!(loaded.validation_commands.len(), 1);
    assert!(loaded.validation_commands[0].passed);
}
