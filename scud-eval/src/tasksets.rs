use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSet {
    pub name: String,
    pub description: String,
    pub task_type: TaskSetType,
    pub scg_content: String,         // The actual SCG file content
    pub expected_files: Vec<String>, // Files that should be created/modified
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskSetType {
    Synthetic, // Controlled, designed for comparison
    Real,      // From actual project, external validity
}

/// Built-in task sets
pub fn builtin_tasksets() -> Vec<TaskSet> {
    vec![
        trivial_taskset(),
        moderate_taskset(),
        complex_taskset(),
        real_scud_taskset(),
    ]
}

fn trivial_taskset() -> TaskSet {
    TaskSet {
        name: "eval-trivial".to_string(),
        description: "5 independent complexity-1 tasks".to_string(),
        task_type: TaskSetType::Synthetic,
        scg_content: r#"# SCUD Graph v1
# Phase: eval-trivial

@meta {
  name eval-trivial
  id_format sequential
}

@nodes
1 | Create hello.py with print statement | P | 1 | M
2 | Create goodbye.py with print statement | P | 1 | M
3 | Create utils.py with add function | P | 1 | M
4 | Create constants.py with PI constant | P | 1 | M
5 | Create README.md with project title | P | 1 | M

@edges

@details
1 | description |
  Create a file hello.py that prints "Hello, World!"
2 | description |
  Create a file goodbye.py that prints "Goodbye, World!"
3 | description |
  Create a file utils.py with a function add(a, b) that returns a + b
4 | description |
  Create a file constants.py that defines PI = 3.14159
5 | description |
  Create a README.md with a single H1 header "Eval Project"
"#
        .to_string(),
        expected_files: vec![
            "hello.py".to_string(),
            "goodbye.py".to_string(),
            "utils.py".to_string(),
            "constants.py".to_string(),
            "README.md".to_string(),
        ],
    }
}

fn moderate_taskset() -> TaskSet {
    TaskSet {
        name: "eval-moderate".to_string(),
        description: "5 tasks with dependencies, complexity 3-5".to_string(),
        task_type: TaskSetType::Synthetic,
        scg_content: r#"# SCUD Graph v1
# Phase: eval-moderate

@meta {
  name eval-moderate
  id_format sequential
}

@nodes
1 | Create data models module | P | 3 | H
2 | Implement CRUD operations | P | 5 | H
3 | Add validation logic | P | 3 | M
4 | Create CLI interface | P | 5 | M
5 | Write unit tests | P | 3 | M

@edges
2 -> 1
3 -> 1
4 -> 2
5 -> 2
5 -> 3

@details
1 | description |
  Create models.py with User and Item dataclasses.
  User: id (int), name (str), email (str)
  Item: id (int), name (str), price (float), owner_id (int)
1 | test_strategy |
  Verify classes can be instantiated with valid data
2 | description |
  Create crud.py with in-memory storage and functions:
  - create_user(user: User) -> User
  - get_user(id: int) -> Optional[User]
  - create_item(item: Item) -> Item
  - get_items_by_owner(owner_id: int) -> List[Item]
2 | test_strategy |
  Test each CRUD operation with valid and invalid inputs
3 | description |
  Create validators.py with:
  - validate_email(email: str) -> bool (basic @ check)
  - validate_price(price: float) -> bool (must be positive)
  Add validation to crud.py create functions
3 | test_strategy |
  Test validators with valid and invalid inputs
4 | description |
  Create cli.py with argparse interface:
  - add-user --name NAME --email EMAIL
  - add-item --name NAME --price PRICE --owner OWNER_ID
  - list-items --owner OWNER_ID
4 | test_strategy |
  Test CLI commands work from command line
5 | description |
  Create test_all.py with pytest tests for:
  - Model instantiation
  - CRUD operations
  - Validators
  Run with: pytest test_all.py
5 | test_strategy |
  All tests should pass with pytest
"#
        .to_string(),
        expected_files: vec![
            "models.py".to_string(),
            "crud.py".to_string(),
            "validators.py".to_string(),
            "cli.py".to_string(),
            "test_all.py".to_string(),
        ],
    }
}

fn complex_taskset() -> TaskSet {
    TaskSet {
        name: "eval-complex".to_string(),
        description: "8 tasks with deep dependencies, complexity 5-13".to_string(),
        task_type: TaskSetType::Synthetic,
        scg_content: r#"# SCUD Graph v1
# Phase: eval-complex

@meta {
  name eval-complex
  id_format sequential
}

@nodes
1 | Design database schema | P | 5 | C
2 | Implement ORM models | P | 8 | H
3 | Create repository layer | P | 8 | H
4 | Build service layer | P | 8 | H
5 | Implement REST API | P | 8 | H
6 | Add authentication | P | 13 | H
7 | Create integration tests | P | 8 | M
8 | Write API documentation | P | 5 | L

@edges
2 -> 1
3 -> 2
4 -> 3
5 -> 4
6 -> 5
7 -> 5
7 -> 6
8 -> 5

@details
1 | description |
  Create schema.sql with tables:
  - users (id, username, email, password_hash, created_at)
  - posts (id, user_id, title, content, created_at, updated_at)
  - comments (id, post_id, user_id, content, created_at)
  Include foreign keys and indexes
2 | description |
  Create models/ directory with SQLAlchemy models:
  - models/user.py - User model
  - models/post.py - Post model
  - models/comment.py - Comment model
  - models/__init__.py - exports all models
3 | description |
  Create repositories/ directory:
  - repositories/base.py - BaseRepository with CRUD
  - repositories/user.py - UserRepository
  - repositories/post.py - PostRepository
  - repositories/comment.py - CommentRepository
4 | description |
  Create services/ directory:
  - services/user.py - user registration, profile
  - services/post.py - create/edit/delete posts
  - services/comment.py - add/remove comments
5 | description |
  Create api/ directory with FastAPI routes:
  - api/users.py - /users endpoints
  - api/posts.py - /posts endpoints
  - api/comments.py - /comments endpoints
  - api/main.py - FastAPI app with routers
6 | description |
  Add authentication:
  - api/auth.py - /login, /register endpoints
  - auth/jwt.py - JWT token creation/validation
  - auth/dependencies.py - FastAPI auth dependencies
  Protect POST/PUT/DELETE endpoints
7 | description |
  Create tests/ directory:
  - tests/conftest.py - fixtures, test database
  - tests/test_users.py - user API tests
  - tests/test_posts.py - post API tests
  - tests/test_auth.py - authentication tests
8 | description |
  Create docs/ directory:
  - docs/api.md - API endpoint documentation
  - docs/setup.md - Installation and setup guide
"#
        .to_string(),
        expected_files: vec![
            "schema.sql".to_string(),
            "models/__init__.py".to_string(),
            "repositories/base.py".to_string(),
            "services/user.py".to_string(),
            "api/main.py".to_string(),
            "auth/jwt.py".to_string(),
            "tests/conftest.py".to_string(),
            "docs/api.md".to_string(),
        ],
    }
}

fn real_scud_taskset() -> TaskSet {
    TaskSet {
        name: "eval-real-scud".to_string(),
        description: "Real tasks: add summary stats to SCUD".to_string(),
        task_type: TaskSetType::Real,
        scg_content: r#"# SCUD Graph v1
# Phase: eval-real-scud

@meta {
  name eval-real-scud
  id_format sequential
}

@nodes
1 | Add task duration tracking | P | 5 | H
2 | Store completion timestamps | P | 3 | H
3 | Calculate average task duration | P | 3 | M
4 | Add stats subcommand | P | 5 | M
5 | Display duration in task list | P | 3 | L

@edges
2 -> 1
3 -> 2
4 -> 3
5 -> 2

@details
1 | description |
  Add started_at and completed_at fields to Task struct in scud-core/src/models/task.rs.
  Update SCG format to persist these timestamps.
2 | description |
  Update set-status command to record timestamps:
  - When status changes to in-progress, set started_at
  - When status changes to done, set completed_at
3 | description |
  Add duration calculation to PhaseStats:
  - average_duration_secs: Option<f64>
  - total_duration_secs: Option<f64>
  Calculate from tasks that have both timestamps
4 | description |
  Add 'scud stats' subcommand that displays:
  - Total tasks by status
  - Average completion time
  - Tasks completed today/this week
5 | description |
  Update 'scud list' to optionally show duration:
  - Add --show-duration flag
  - Display elapsed time for in-progress tasks
  - Display total time for completed tasks
"#
        .to_string(),
        expected_files: vec![
            "scud-core/src/models/task.rs".to_string(),
            "scud-cli/src/commands/set_status.rs".to_string(),
            "scud-cli/src/commands/stats.rs".to_string(),
            "scud-cli/src/commands/list.rs".to_string(),
        ],
    }
}

/// Install a taskset to ~/.scud-eval/tasksets/
pub fn install_taskset(taskset: &TaskSet) -> Result<PathBuf> {
    let dir = super::storage::tasksets_dir().join(&taskset.name);
    std::fs::create_dir_all(&dir)?;

    // Write SCG file
    let scg_path = dir.join("tasks.scg");
    std::fs::write(&scg_path, &taskset.scg_content)?;

    // Write metadata
    let meta_path = dir.join("taskset.json");
    let meta = serde_json::to_string_pretty(taskset)?;
    std::fs::write(&meta_path, meta)?;

    Ok(dir)
}
