use serde::Serialize;

use crate::model::page::Page;

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSettings {
    pub default_page: Option<Page>,
}

pub struct ProjectSettingsManager {
    pub project_settings: ProjectSettings,
}

impl ProjectSettingsManager {
    pub fn new() -> ProjectSettingsManager {
        ProjectSettingsManager {
            project_settings: ProjectSettings { default_page: None },
        }
    }
}
