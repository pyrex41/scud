use serde::{Deserialize, Serialize};

/// Epic Group - for coordinating related epics (e.g., backend/frontend)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpicGroup {
    pub id: String,
    pub name: String,
    pub epic_tags: Vec<String>,
    pub description: Option<String>,
    pub created_at: String,
    pub status: GroupStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GroupStatus {
    Active,
    Completed,
    Archived,
}

impl EpicGroup {
    pub fn new(id: String, name: String, epic_tags: Vec<String>) -> Self {
        EpicGroup {
            id,
            name,
            epic_tags,
            description: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            status: GroupStatus::Active,
        }
    }

    pub fn contains_epic(&self, tag: &str) -> bool {
        self.epic_tags.iter().any(|t| t == tag)
    }

    pub fn add_epic(&mut self, tag: String) {
        if !self.contains_epic(&tag) {
            self.epic_tags.push(tag);
        }
    }

    pub fn remove_epic(&mut self, tag: &str) -> bool {
        if let Some(pos) = self.epic_tags.iter().position(|t| t == tag) {
            self.epic_tags.remove(pos);
            true
        } else {
            false
        }
    }
}

/// Groups collection stored in .taskmaster/epic-groups.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EpicGroups {
    pub groups: Vec<EpicGroup>,
}

impl EpicGroups {
    pub fn new() -> Self {
        EpicGroups {
            groups: Vec::new(),
        }
    }

    pub fn add_group(&mut self, group: EpicGroup) {
        self.groups.push(group);
    }

    pub fn get_group(&self, id: &str) -> Option<&EpicGroup> {
        self.groups.iter().find(|g| g.id == id)
    }

    pub fn get_group_mut(&mut self, id: &str) -> Option<&mut EpicGroup> {
        self.groups.iter_mut().find(|g| g.id == id)
    }

    pub fn find_group_for_epic(&self, epic_tag: &str) -> Option<&EpicGroup> {
        self.groups.iter().find(|g| g.contains_epic(epic_tag))
    }

    pub fn remove_group(&mut self, id: &str) -> Option<EpicGroup> {
        self.groups
            .iter()
            .position(|g| g.id == id)
            .map(|idx| self.groups.remove(idx))
    }
}
