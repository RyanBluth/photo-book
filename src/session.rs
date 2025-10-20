use std::path::PathBuf;

use egui::modal;
use log::error;

use crate::{
    auto_persisting::AutoPersisting,
    config::{Config, ConfigModification},
    dependencies::{Dependency, Singleton, SingletonFor},
    modal::{
        ModalActionResponse,
        manager::{ModalManager, TypedModalId},
        save_warning::{SaveWarningModal, SaveWarningResponse, SaveWarningSource},
    },
    photo_manager::PhotoManager,
    project::{Project, ProjectError},
    scene::{organize_edit_scene::OrganizeEditScene, organize_scene::GalleryScene},
};

#[derive(Debug, Clone)]
pub enum PendingOperation {
    NewProject,
    LoadProject(Option<PathBuf>),
}

#[derive(Debug)]
pub enum SessionError {
    ProjectError(ProjectError),
    DialogCancelled,
    DialogError(native_dialog::Error),
    WaitingForUserInput,
}

impl From<ProjectError> for SessionError {
    fn from(err: ProjectError) -> Self {
        SessionError::ProjectError(err)
    }
}

impl From<native_dialog::Error> for SessionError {
    fn from(err: native_dialog::Error) -> Self {
        SessionError::DialogError(err)
    }
}

pub struct Session {
    pub active_project: Option<PathBuf>,
    save_warning_modal_id: Option<TypedModalId<SaveWarningModal>>,
    pending_operation: Option<PendingOperation>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            active_project: None,
            save_warning_modal_id: None,
            pending_operation: None,
        }
    }

    pub fn check_modals(&mut self, current_scene: &OrganizeEditScene) -> Option<OrganizeEditScene> {
        let modal_id = self.save_warning_modal_id.as_ref()?.clone();

        let response = Dependency::<ModalManager>::get()
            .with_lock(|modal_manager| modal_manager.response_for(&modal_id));

        match response {
            Ok(Some(SaveWarningResponse::Save)) => {
                if let Err(e) = self.save_project(current_scene) {
                    log::error!(
                        "Error saving project before executing pending operation: {:?}",
                        e
                    );
                    self.pending_operation = None;
                    return None;
                }

                let result = self.execute_pending_operation();

                Dependency::<ModalManager>::get().with_lock_mut(|modal_manager| {
                    modal_manager.dismiss(modal_id);
                });

                return result;
            }
            Ok(Some(SaveWarningResponse::DontSave)) => {
                let result = self.execute_pending_operation();

                Dependency::<ModalManager>::get().with_lock_mut(|modal_manager| {
                    modal_manager.dismiss(modal_id);
                });

                return result;
            }
            Ok(Some(SaveWarningResponse::Cancel)) => {
                Dependency::<ModalManager>::get().with_lock_mut(|modal_manager| {
                    modal_manager.dismiss(modal_id);
                });
                self.pending_operation = None;
            }
            Err(err) => {
                error!(
                    "Error occurred while handling modal response for save warning dialog: {}",
                    err
                );
            }
            _ => {}
        }
        None
    }

    pub fn load_project(
        &mut self,
        path: Option<PathBuf>,
    ) -> Result<OrganizeEditScene, SessionError> {
        if self.has_unsaved_changes() {
            self.pending_operation = Some(PendingOperation::LoadProject(path.clone()));
            self.save_warning_modal_id = Some(ModalManager::push(SaveWarningModal::new(
                SaveWarningSource::LoadProject(path),
            )));
            return Err(SessionError::WaitingForUserInput);
        }

        self.load_project_internal(path)
    }

    pub fn save_project(&mut self, scene: &OrganizeEditScene) -> Result<(), SessionError> {
        let path = match &self.active_project {
            Some(p) => p.clone(),
            None => {
                let save_path = native_dialog::DialogBuilder::file()
                    .add_filter("Photobook Project", &["rpb"])
                    .save_single_file()
                    .show()?;

                match save_path {
                    Some(p) => p,
                    None => return Err(SessionError::DialogCancelled),
                }
            }
        };

        Project::save(&path, scene)?;
        Dependency::<AutoPersisting<Config>>::get().with_lock_mut(|config| {
            let _ = config.modify(ConfigModification::AddRecentProject(path.clone()));
            let _ = config.modify(ConfigModification::SetLastProject(path.clone()));
        });

        self.active_project = Some(path.clone());
        Ok(())
    }

    pub fn new_project(&mut self) -> Result<OrganizeEditScene, SessionError> {
        if self.has_unsaved_changes() {
            self.pending_operation = Some(PendingOperation::NewProject);
            self.save_warning_modal_id = Some(ModalManager::push(SaveWarningModal::new(
                SaveWarningSource::NewProject,
            )));
            return Err(SessionError::WaitingForUserInput);
        }

        self.new_project_internal()
    }

    fn execute_pending_operation(&mut self) -> Option<OrganizeEditScene> {
        match self.pending_operation.take() {
            Some(PendingOperation::NewProject) => match self.new_project_internal() {
                Ok(scene) => Some(scene),
                Err(e) => {
                    log::error!("Error creating new project: {:?}", e);
                    None
                }
            },
            Some(PendingOperation::LoadProject(path)) => match self.load_project_internal(path) {
                Ok(scene) => Some(scene),
                Err(e) => {
                    log::error!("Error loading project: {:?}", e);
                    None
                }
            },
            None => None,
        }
    }

    fn load_project_internal(
        &mut self,
        path: Option<PathBuf>,
    ) -> Result<OrganizeEditScene, SessionError> {
        let path = match path {
            Some(p) => p,
            None => {
                let open_path = native_dialog::DialogBuilder::file()
                    .add_filter("Photobook Project", &["rpb"])
                    .open_single_file()
                    .show()?;

                match open_path {
                    Some(p) => p,
                    None => return Err(SessionError::DialogCancelled),
                }
            }
        };
        Dependency::<PhotoManager>::get().with_lock_mut(|photo_manager| {
            photo_manager.clear();
        });

        let scene = Project::load(&path)?;

        let config: Singleton<AutoPersisting<Config>> = Dependency::get();
        config.with_lock_mut(|config| {
            let _ = config.modify(ConfigModification::AddRecentProject(path.clone()));
            let _ = config.modify(ConfigModification::SetLastProject(path.clone()));
        });

        self.active_project = Some(path.clone());

        Ok(scene)
    }

    fn new_project_internal(&mut self) -> Result<OrganizeEditScene, SessionError> {
        self.active_project = None;
        let scene = OrganizeEditScene::new(GalleryScene::new(), None);

        Dependency::<PhotoManager>::get().with_lock_mut(|photo_manager| {
            photo_manager.clear();
        });

        Ok(scene)
    }

    fn has_unsaved_changes(&self) -> bool {
        Dependency::<PhotoManager>::get().with_lock(|photo_manager| photo_manager.has_photos())
    }
}
