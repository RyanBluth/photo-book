use std::path::PathBuf;

use log::{error, info};

use crate::{
    dependencies::{Dependency, SingletonFor},
    photo_manager::PhotoManager,
    project::v1::{Project, ProjectError},
    scene::organize_edit_scene::OrganizeEditScene,
    session::Session,
};

#[derive(Debug, thiserror::Error)]
pub enum AutoSaveManagerError {
    #[error("Failed to save auto save: {0}")]
    ProjectError(#[from] ProjectError),

    #[error("Failed to get auto save path")]
    AutoSavePathError,

    #[error("Save task already in progress")]
    SaveTaskInProgress,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AutoSave {
    // The active project when the auto save was created
    active_project: Option<PathBuf>,
    project: Project,
}

pub struct AutoSaveManager {
    last_save_time: Option<std::time::Instant>,
    current_save_task: Option<tokio::task::JoinHandle<()>>,
}

impl AutoSaveManager {
    pub fn new() -> Self {
        Self {
            last_save_time: None,
            current_save_task: None,
        }
    }

    pub fn load_auto_save() -> Option<OrganizeEditScene> {
        let path = auto_save_path()?;
        let data = match std::fs::read_to_string(path) {
            Ok(data) => data,
            Err(err) => {
                error!("Error loading auto save: {:?}", err);
                return None;
            }
        };

        let auto_save: AutoSave = match serde_json::from_str(&data) {
            Ok(save) => save,
            Err(err) => {
                error!("Error loading auto save: {:?}", err);
                return None;
            }
        };

        if let Some(active_project) = auto_save.active_project {
            Dependency::<Session>::get().with_lock_mut(|session| {
                session.active_project = Some(active_project);
            });
        }

        Some(auto_save.project.into())
    }

    pub fn auto_save_if_needed(
        &mut self,
        root_scene: &OrganizeEditScene,
    ) -> Result<(), AutoSaveManagerError> {
        let now = std::time::Instant::now();
        let should_save = match self.last_save_time {
            None => true,
            Some(last_save_time) => (now - last_save_time).as_secs() > 5,
        };

        if should_save {
            self.auto_save(root_scene)?;
        }

        Ok(())
    }

    pub fn auto_save(
        &mut self,
        root_scene: &OrganizeEditScene,
    ) -> Result<(), AutoSaveManagerError> {
        if let Some(task) = &self.current_save_task {
            if !task.is_finished() {
                return Err(AutoSaveManagerError::SaveTaskInProgress);
            }
        }

        let path = auto_save_path().ok_or(AutoSaveManagerError::AutoSavePathError)?;
        self.current_save_task = Some(create_save_task(root_scene.clone(), path));
        self.last_save_time = Some(std::time::Instant::now());

        Ok(())
    }

    pub fn get_auto_save_modification_time() -> Option<std::time::SystemTime> {
        let path = auto_save_path()?;
        std::fs::metadata(path).ok()?.modified().ok()
    }
}

fn create_save_task(root_scene: OrganizeEditScene, path: PathBuf) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        info!("Auto saving project to {}", path.display());

        let auto_save: AutoSave =
            Dependency::<PhotoManager>::get().with_lock(|photo_manager| AutoSave {
                active_project: Dependency::<Session>::get()
                    .with_lock(|session| session.active_project.clone()),
                project: Project::new(&root_scene, &photo_manager),
            });

        let data = match serde_json::to_string_pretty(&auto_save) {
            Ok(data) => data,
            Err(err) => {
                error!("Error saving auto save: {:?}", err);
                return;
            }
        };

        if let Err(e) = std::fs::write(path, data) {
            error!("Error saving auto save: {:?}", e);
        }
    })
}

fn auto_save_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|cache_dir| cache_dir.join("auto_save.json"))
}
