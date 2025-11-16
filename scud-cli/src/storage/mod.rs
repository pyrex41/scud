use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::models::{Epic, WorkflowState};

pub struct Storage {
    project_root: PathBuf,
}

impl Storage {
    pub fn new(project_root: Option<PathBuf>) -> Self {
        let root = project_root.unwrap_or_else(|| std::env::current_dir().unwrap());
        Storage {
            project_root: root,
        }
    }

    pub fn taskmaster_dir(&self) -> PathBuf {
        self.project_root.join(".taskmaster")
    }

    pub fn tasks_file(&self) -> PathBuf {
        self.taskmaster_dir().join("tasks").join("tasks.json")
    }

    pub fn workflow_file(&self) -> PathBuf {
        self.taskmaster_dir().join("workflow-state.json")
    }

    pub fn docs_dir(&self) -> PathBuf {
        self.project_root.join("docs")
    }

    pub fn is_initialized(&self) -> bool {
        self.taskmaster_dir().exists() && self.tasks_file().exists() && self.workflow_file().exists()
    }

    pub fn initialize(&self) -> Result<()> {
        // Create .taskmaster directory structure
        let taskmaster = self.taskmaster_dir();
        fs::create_dir_all(taskmaster.join("tasks"))
            .context("Failed to create .taskmaster/tasks directory")?;

        // Initialize tasks.json with empty object
        let tasks_file = self.tasks_file();
        if !tasks_file.exists() {
            let empty_tasks: HashMap<String, Epic> = HashMap::new();
            self.save_tasks(&empty_tasks)?;
        }

        // Initialize workflow-state.json
        let workflow_file = self.workflow_file();
        if !workflow_file.exists() {
            let workflow_state = WorkflowState::new();
            self.save_workflow_state(&workflow_state)?;
        }

        // Create docs directories
        let docs = self.docs_dir();
        fs::create_dir_all(docs.join("prd"))?;
        fs::create_dir_all(docs.join("epics"))?;
        fs::create_dir_all(docs.join("architecture"))?;
        fs::create_dir_all(docs.join("retrospectives"))?;

        // Update .gitignore
        self.update_gitignore()?;

        Ok(())
    }

    fn update_gitignore(&self) -> Result<()> {
        let gitignore_path = self.project_root.join(".gitignore");
        let entry = "\n# SCUD Task Master\n.taskmaster/\n";

        if gitignore_path.exists() {
            let content = fs::read_to_string(&gitignore_path)?;
            if !content.contains(".taskmaster/") {
                fs::write(&gitignore_path, format!("{}{}", content, entry))?;
            }
        } else {
            fs::write(&gitignore_path, entry)?;
        }

        Ok(())
    }

    pub fn load_tasks(&self) -> Result<HashMap<String, Epic>> {
        let path = self.tasks_file();
        if !path.exists() {
            anyhow::bail!(
                "Tasks file not found: {}\nRun: scud init",
                path.display()
            );
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read tasks from {}", path.display()))?;

        let tasks: HashMap<String, Epic> = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse tasks.json"))?;

        Ok(tasks)
    }

    pub fn save_tasks(&self, tasks: &HashMap<String, Epic>) -> Result<()> {
        let path = self.tasks_file();
        let dir = path.parent().unwrap();

        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }

        let content = serde_json::to_string_pretty(tasks)?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write tasks to {}", path.display()))?;

        Ok(())
    }

    pub fn load_workflow_state(&self) -> Result<WorkflowState> {
        let path = self.workflow_file();
        if !path.exists() {
            anyhow::bail!(
                "Workflow state not found: {}\nRun: scud init",
                path.display()
            );
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read workflow state from {}", path.display()))?;

        let state: WorkflowState = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse workflow-state.json"))?;

        Ok(state)
    }

    pub fn save_workflow_state(&self, state: &WorkflowState) -> Result<()> {
        let path = self.workflow_file();
        let content = serde_json::to_string_pretty(state)?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write workflow state to {}", path.display()))?;

        Ok(())
    }

    pub fn get_active_epic(&self) -> Result<Option<String>> {
        let state = self.load_workflow_state()?;
        Ok(state.active_epic)
    }

    pub fn set_active_epic(&self, epic_tag: &str) -> Result<()> {
        let tasks = self.load_tasks()?;
        if !tasks.contains_key(epic_tag) {
            anyhow::bail!("Epic '{}' not found", epic_tag);
        }

        let mut state = self.load_workflow_state()?;
        state.active_epic = Some(epic_tag.to_string());
        state.update();
        self.save_workflow_state(&state)?;

        Ok(())
    }

    pub fn read_file(&self, path: &Path) -> Result<String> {
        fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))
    }
}
