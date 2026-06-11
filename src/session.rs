use std::path::PathBuf;

use log::error;

use crate::{
    auto_persisting::AutoPersisting,
    config::{Config, ConfigModification},
    dep, dep_mut,
    modal::{
        manager::{ModalManager, TypedModalId},
        save_warning::{SaveWarningModal, SaveWarningResponse, SaveWarningSource},
    },
    photo_manager::PhotoManager,
    project::{Project, ProjectError},
    scene::{gallery_scene::GalleryScene, organize_edit_scene::OrganizeEditScene},
    selection_manager::SelectionManager,
};

#[derive(Debug, Clone)]
pub enum PendingOperation {
    NewProject,
    LoadProject(Option<PathBuf>),
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum SessionError {
    _ProjectError(ProjectError),
    DialogCancelled,
    _DialogError(native_dialog::Error),
    WaitingForUserInput,
}

impl From<ProjectError> for SessionError {
    fn from(err: ProjectError) -> Self {
        SessionError::_ProjectError(err)
    }
}

impl From<native_dialog::Error> for SessionError {
    fn from(err: native_dialog::Error) -> Self {
        SessionError::_DialogError(err)
    }
}

pub struct Session {
    pub active_project: Option<PathBuf>,
    saved_project: Option<Project>,
    save_warning_modal_id: Option<TypedModalId<SaveWarningModal>>,
    pending_operation: Option<PendingOperation>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            active_project: None,
            saved_project: None,
            save_warning_modal_id: None,
            pending_operation: None,
        }
    }

    pub fn check_modals(&mut self, current_scene: &OrganizeEditScene) -> Option<OrganizeEditScene> {
        let modal_id = self.save_warning_modal_id.as_ref()?.clone();

        let response = dep!(ModalManager, |modal_manager| modal_manager
            .response_for(&modal_id));

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

                dep_mut!(ModalManager, |modal_manager| {
                    modal_manager.dismiss(modal_id);
                });

                return result;
            }
            Ok(Some(SaveWarningResponse::DontSave)) => {
                let result = self.execute_pending_operation();

                dep_mut!(ModalManager, |modal_manager| {
                    modal_manager.dismiss(modal_id);
                });

                return result;
            }
            Ok(Some(SaveWarningResponse::Cancel)) => {
                dep_mut!(ModalManager, |modal_manager| {
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
        current_scene: &OrganizeEditScene,
        path: Option<PathBuf>,
    ) -> Result<OrganizeEditScene, SessionError> {
        if self.has_unsaved_changes(current_scene) {
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
                    .add_filter("Photo Book Collection", &["rpb"])
                    .save_single_file()
                    .show()?;

                match save_path {
                    Some(p) => p,
                    None => return Err(SessionError::DialogCancelled),
                }
            }
        };

        let project = Project::new(scene);
        Project::save_project(&path, &project)?;
        dep_mut!(AutoPersisting<Config>, |config| {
            let _ = config.modify(ConfigModification::AddRecentProject(path.clone()));
            let _ = config.modify(ConfigModification::SetLastProject(path.clone()));
        });

        self.saved_project = Some(project);
        self.active_project = Some(path.clone());
        Ok(())
    }

    pub fn mark_project_loaded(&mut self, path: PathBuf, project: Project) {
        self.active_project = Some(path);
        self.saved_project = Some(project);
    }

    pub fn new_project(
        &mut self,
        current_scene: &OrganizeEditScene,
    ) -> Result<OrganizeEditScene, SessionError> {
        if self.has_unsaved_changes(current_scene) {
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
                    .add_filter("Photo Book Collection", &["rpb"])
                    .open_single_file()
                    .show()?;

                match open_path {
                    Some(p) => p,
                    None => return Err(SessionError::DialogCancelled),
                }
            }
        };
        dep_mut!(PhotoManager, |photo_manager| {
            photo_manager.clear();
        });
        dep_mut!(SelectionManager, |selection_manager| selection_manager
            .clear());

        let project = Project::load_project(&path)?;
        let scene = project.clone().into();

        dep_mut!(AutoPersisting<Config>, |config| {
            let _ = config.modify(ConfigModification::AddRecentProject(path.clone()));
            let _ = config.modify(ConfigModification::SetLastProject(path.clone()));
        });

        self.mark_project_loaded(path.clone(), project);

        Ok(scene)
    }

    fn new_project_internal(&mut self) -> Result<OrganizeEditScene, SessionError> {
        self.active_project = None;
        self.saved_project = None;
        let scene = OrganizeEditScene::new(GalleryScene::new(), None);

        dep_mut!(PhotoManager, |photo_manager| {
            photo_manager.clear();
        });
        dep_mut!(SelectionManager, |selection_manager| selection_manager
            .clear());

        Ok(scene)
    }

    fn has_unsaved_changes(&self, current_scene: &OrganizeEditScene) -> bool {
        if !self.has_project_content(current_scene) {
            return false;
        }

        let current_project = Project::new(current_scene);
        self.saved_project
            .as_ref()
            .map(|saved_project| saved_project != &current_project)
            .unwrap_or(true)
    }

    fn has_project_content(&self, current_scene: &OrganizeEditScene) -> bool {
        dep!(PhotoManager, |photo_manager| photo_manager.has_photos()) || current_scene.has_books()
    }
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use super::*;
    use crate::{
        id::next_page_id,
        model::{edit_state::EditablePage, page::Page, unit::Unit},
        scene::organize_edit_scene::Book,
        widget::canvas::CanvasState,
    };

    fn scene_with_book(width: f32, height: f32) -> OrganizeEditScene {
        let page_id = next_page_id();
        let mut pages = IndexMap::new();
        pages.insert(
            page_id,
            CanvasState::with_layers(
                IndexMap::new(),
                EditablePage::new(Page::new(width, height, 300, Unit::Inches)),
                None,
                Vec::new(),
            ),
        );

        let state = crate::scene::canvas_scene::CanvasSceneState::with_pages(pages, page_id);
        OrganizeEditScene::with_books(
            GalleryScene::new(),
            vec![Book::with_state(
                "book-1".to_string(),
                "Book 1".to_string(),
                state,
            )],
        )
    }

    #[test]
    fn saved_book_only_collection_is_not_unsaved() {
        dep_mut!(PhotoManager, |photo_manager| photo_manager.clear());
        let scene = scene_with_book(6.0, 8.0);
        let mut session = Session::new();
        session.saved_project = Some(Project::new(&scene));

        assert!(!session.has_unsaved_changes(&scene));
    }

    #[test]
    fn modified_book_collection_is_unsaved() {
        dep_mut!(PhotoManager, |photo_manager| photo_manager.clear());
        let saved_scene = scene_with_book(6.0, 8.0);
        let modified_scene = scene_with_book(10.0, 12.0);
        let mut session = Session::new();
        session.saved_project = Some(Project::new(&saved_scene));

        assert!(session.has_unsaved_changes(&modified_scene));
    }
}
