use std::path::PathBuf;

use log::{error, info};

use crate::{
    auto_persisting::{AutoPersisting, PersistentModifiable},
    dependencies::{Dependency, Singleton, SingletonFor},
    photo_manager::PhotoManager,
    project::v1::{Project, ProjectError},
    scene::organize_edit_scene::OrganizeEditScene,
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

    pub fn load_auto_save() -> Result<Option<OrganizeEditScene>, AutoSaveManagerError> {
        match auto_save_path() {
            Some(path) => {
                let project = Project::load(&path)?;
                return Ok(Some(project));
            }
            None => {
                return Err(AutoSaveManagerError::AutoSavePathError);
            }
        }
    }

    pub fn auto_save_if_needed(
        &mut self,
        root_scene: &OrganizeEditScene,
    ) -> Result<(), AutoSaveManagerError> {
        let now = std::time::Instant::now();
        let time_since_last_save = self
            .last_save_time
            .map(|last_save_time| now - last_save_time);

        if time_since_last_save.is_none() || time_since_last_save.unwrap().as_secs() > 5 {
            self.auto_save(root_scene)?;
        }

        Ok(())
    }

    pub fn auto_save(
        &mut self,
        root_scene: &OrganizeEditScene,
    ) -> Result<(), AutoSaveManagerError> {
        if let Some(current_save_task) = self.current_save_task.take() {
            if !current_save_task.is_finished() {
                error!("Auto save already in progress, skipping this save");
            }
            return Err(AutoSaveManagerError::SaveTaskInProgress);
        }

        let path = auto_save_path().ok_or_else(|| AutoSaveManagerError::AutoSavePathError)?;
        let cloned_root_scene = root_scene.clone();

        self.current_save_task = Some(tokio::spawn(async move {
            info!("Auto saving project to {}", path.display());

            let save_result = Dependency::<PhotoManager>::get().with_lock(|photo_manager| {
                Project::save(&path, &cloned_root_scene, &photo_manager)
            });

            if let Err(err) = save_result {
                error!("Error saving auto save: {:?}", err);
            }
        }));

        let now = std::time::Instant::now();
        self.last_save_time = Some(now);

        Ok(())
    }
}

fn auto_save_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|cache_dir| cache_dir.join("auto_save.json"))
}
