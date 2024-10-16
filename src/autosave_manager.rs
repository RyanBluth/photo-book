use std::path::PathBuf;

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
}

pub struct AutoSaveManager {}

impl AutoSaveManager {

    pub fn new() -> Self {
        Self {}
    }

    pub fn load_auto_save() -> Result<Option<OrganizeEditScene>, AutoSaveManagerError> {
        match auto_save_path() {
            Some(path) => {
                let photo_manager: Singleton<PhotoManager> = Dependency::get();
                let project = photo_manager
                    .with_lock_mut(|photo_manager| Project::load(&path, photo_manager))?;

                return Ok(Some(project));
            }
            None => {
                return Err(AutoSaveManagerError::AutoSavePathError);
            }
        }
    }

    pub fn auto_save(&self, root_scene: OrganizeEditScene) -> Result<(), AutoSaveManagerError> {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        let path = auto_save_path().ok_or_else(|| AutoSaveManagerError::AutoSavePathError)?;

        let project = photo_manager
            .with_lock_mut(|photo_manager| Project::save(&path, &root_scene, photo_manager))?;

        Ok(())
    }
}

fn auto_save_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|cache_dir| cache_dir.join("auto_save.json"))
}
