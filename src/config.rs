use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

use savefile_derive::Savefile;

use crate::project::NamedWorkflow;
use crate::{auto_persisting::PersistentModifiable, dirs::Dirs};

const CONFIG_VERSION: u32 = 0;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Savefile error: {0}")]
    Savefile(#[from] savefile::SavefileError),
}

#[derive(Debug, Savefile, Default)]
pub struct Config {
    recent_projects: Option<Vec<PathBuf>>,
    last_project: Option<PathBuf>,
    import_workflows: Vec<NamedWorkflow>,
}

pub enum ConfigModification {
    AddRecentProject(PathBuf),
    SetLastProject(PathBuf),
    SaveImportWorkflow(NamedWorkflow),
}

impl Config {
    fn load_from_directory(directory: &Path) -> Result<Self, ConfigError> {
        let config_path = directory.join("config.bin");
        match File::open(&config_path) {
            Ok(mut file) => Ok(savefile::load(&mut file, CONFIG_VERSION)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    fn save_to_path(&self, path: &Path) -> Result<(), ConfigError> {
        // Serialize before opening the file so serialization errors cannot truncate it.
        let contents = savefile::save_to_mem(CONFIG_VERSION, self)?;
        let mut file = File::create(path)?;
        file.write_all(&contents)?;
        Ok(())
    }

    pub fn import_workflows(&self) -> &[NamedWorkflow] {
        &self.import_workflows
    }

    pub fn recent_projects(&self) -> &[PathBuf] {
        self.recent_projects.as_deref().unwrap_or(&[])
    }

    pub fn last_project(&self) -> Option<&PathBuf> {
        self.last_project.as_ref()
    }
}

impl PersistentModifiable<Config> for Config {
    type Error = ConfigError;
    type Modification = ConfigModification;

    fn load() -> Result<Config, ConfigError> {
        Self::load_from_directory(&Dirs::Config.path())
    }

    fn save(&self) -> Result<(), ConfigError> {
        self.save_to_path(&Dirs::Config.path().join("config.bin"))
    }

    fn modify(&mut self, modification: ConfigModification) -> Result<(), ConfigError> {
        match modification {
            ConfigModification::AddRecentProject(project) => {
                if let Some(recent_projects) = &mut self.recent_projects {
                    if let Some(index) = recent_projects.iter().position(|p| p == &project) {
                        recent_projects.remove(index);
                    }
                    recent_projects.insert(0, project);
                } else {
                    self.recent_projects = Some(vec![project]);
                }
            }
            ConfigModification::SetLastProject(path_buf) => {
                self.last_project = Some(path_buf);
            }
            ConfigModification::SaveImportWorkflow(workflow) => {
                let previous = self.import_workflows.clone();
                if let Some(existing) = self
                    .import_workflows
                    .iter_mut()
                    .find(|saved| saved.name == workflow.name)
                {
                    *existing = workflow;
                } else {
                    self.import_workflows.push(workflow);
                }
                if let Err(error) = self.save() {
                    self.import_workflows = previous;
                    return Err(error);
                }
                return Ok(());
            }
        }

        self.save()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{FileWorkflow, SubdirectoryTemplate, WorkflowStep};

    #[test]
    fn savefile_round_trip() {
        let directory = tempfile::tempdir().unwrap();
        let config = Config {
            recent_projects: Some(vec![PathBuf::from("collection.photobook")]),
            last_project: Some(PathBuf::from("collection.photobook")),
            import_workflows: vec![NamedWorkflow {
                name: "Raw import".into(),
                workflow: FileWorkflow {
                    steps: vec![WorkflowStep::AppendSubdirectory(SubdirectoryTemplate {
                        template: "raw/{{iso}}".into(),
                    })],
                },
            }],
        };
        config
            .save_to_path(&directory.path().join("config.bin"))
            .unwrap();
        let loaded = Config::load_from_directory(directory.path()).unwrap();
        assert_eq!(loaded.recent_projects, config.recent_projects);
        assert_eq!(loaded.last_project, config.last_project);
        assert_eq!(loaded.import_workflows, config.import_workflows);
    }

    #[test]
    fn missing_preferences_default_but_corrupt_preferences_error() {
        let directory = tempfile::tempdir().unwrap();
        let loaded = Config::load_from_directory(directory.path()).unwrap();
        assert!(loaded.recent_projects().is_empty());
        assert!(loaded.import_workflows().is_empty());
        std::fs::write(directory.path().join("config.bin"), "corrupt").unwrap();
        assert!(Config::load_from_directory(directory.path()).is_err());
    }
}
