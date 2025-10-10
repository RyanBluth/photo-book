use std::path::PathBuf;

use log::{error, info};
use savefile_derive::Savefile;

use crate::{
    dependencies::{Dependency, SingletonFor},
    photo_manager::PhotoManager,
    project::{PROJECT_VERSION, Project, ProjectError},
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

    #[error("Savefile error: {0}")]
    SavefileError(#[from] savefile::SavefileError),
}

#[derive(Debug, Savefile)]
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

        let auto_save: AutoSave = match savefile::load_file::<AutoSave, _>(path, PROJECT_VERSION) {
            Ok(auto_save) => auto_save,
            Err(e) => {
                error!("Error loading auto save: {:?}", e);
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
        if let Some(task) = &self.current_save_task
            && !task.is_finished()
        {
            return Err(AutoSaveManagerError::SaveTaskInProgress);
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

        let auto_save: AutoSave = AutoSave {
            active_project: Dependency::<Session>::get()
                .with_lock(|session| session.active_project.clone()),
            project: Project::new(&root_scene),
        };

        if let Err(e) = savefile::save_file_compressed(path, PROJECT_VERSION, &auto_save) {
            error!("Error saving auto save: {:?}", e);
        }
    })
}

fn auto_save_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|cache_dir| cache_dir.join("auto_save.rpb"))
}
