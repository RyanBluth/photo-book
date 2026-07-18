use std::path::PathBuf;

use egui::Context;
use log::error;

use crate::{
    auto_persisting::AutoPersisting,
    config::{Config, ConfigModification},
    dep, dep_mut,
    file_dialog::{self, FileDialogResult},
    modal::{
        manager::{ModalManager, TypedModalId},
        save_warning::{SaveWarningModal, SaveWarningResponse, SaveWarningSource},
    },
    photo_manager::PhotoManager,
    project::{Project, ProjectError, ProjectPreferences},
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
    WaitingForUserInput,
}

impl From<ProjectError> for SessionError {
    fn from(err: ProjectError) -> Self {
        SessionError::_ProjectError(err)
    }
}

pub struct Session {
    pub active_project: Option<PathBuf>,
    pub project_preferences: ProjectPreferences,
    saved_project: Option<Project>,
    save_warning_modal_id: Option<TypedModalId<SaveWarningModal>>,
    pending_operation: Option<PendingOperation>,
    file_dialog_open: bool,
    completed_scene: Option<OrganizeEditScene>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            active_project: None,
            project_preferences: ProjectPreferences::default(),
            saved_project: None,
            save_warning_modal_id: None,
            pending_operation: None,
            file_dialog_open: false,
            completed_scene: None,
        }
    }

    pub fn check_modals(
        &mut self,
        current_scene: &OrganizeEditScene,
        ctx: &Context,
    ) -> Option<OrganizeEditScene> {
        if let Some(scene) = self.completed_scene.take() {
            return Some(scene);
        }

        let modal_id = self.save_warning_modal_id?;

        let response = dep!(ModalManager, |modal_manager| modal_manager
            .response_for(&modal_id));

        let scene = match response {
            Ok(Some(SaveWarningResponse::Save)) => match self.save_project(current_scene, ctx) {
                Ok(()) => self.execute_pending_operation(ctx),
                Err(SessionError::WaitingForUserInput) => return None,
                Err(e) => {
                    log::error!(
                        "Error saving project before executing pending operation: {:?}",
                        e
                    );
                    self.pending_operation = None;
                    None
                }
            },
            Ok(Some(SaveWarningResponse::DontSave)) => self.execute_pending_operation(ctx),
            Ok(Some(SaveWarningResponse::Cancel)) => {
                self.pending_operation = None;
                None
            }
            Err(err) => {
                error!(
                    "Error occurred while handling modal response for save warning dialog: {}",
                    err
                );
                return None;
            }
            _ => return None,
        };

        self.dismiss_save_warning(modal_id);
        scene
    }

    pub fn load_project(
        &mut self,
        current_scene: &OrganizeEditScene,
        path: Option<PathBuf>,
        ctx: &Context,
    ) -> Result<OrganizeEditScene, SessionError> {
        if self.file_dialog_open {
            return Err(SessionError::WaitingForUserInput);
        }

        if self.has_unsaved_changes(current_scene) {
            self.pending_operation = Some(PendingOperation::LoadProject(path.clone()));
            self.save_warning_modal_id = Some(ModalManager::push(SaveWarningModal::new(
                SaveWarningSource::LoadProject(path),
            )));
            return Err(SessionError::WaitingForUserInput);
        }

        self.load_project_internal(path, ctx)
    }

    pub fn save_project(
        &mut self,
        scene: &OrganizeEditScene,
        ctx: &Context,
    ) -> Result<(), SessionError> {
        if self.file_dialog_open {
            return Err(SessionError::WaitingForUserInput);
        }

        let path = match &self.active_project {
            Some(p) => p.clone(),
            None => {
                let project =
                    Project::new_with_preferences(scene, self.project_preferences.clone());
                let dialog = native_dialog::DialogBuilder::file()
                    .add_filter("Photo Book Collection", ["rpb"])
                    .save_single_file()
                    .spawn();
                let continuation = self.pending_operation.take();
                self.file_dialog_open = true;
                file_dialog::spawn(dialog, ctx.clone(), move |result, ctx| {
                    dep_mut!(Session, |session| {
                        session.finish_save_dialog(result, project, continuation, ctx);
                    });
                });
                return Err(SessionError::WaitingForUserInput);
            }
        };

        let project = Project::new_with_preferences(scene, self.project_preferences.clone());
        self.save_project_to_path(path, project)
    }

    fn save_project_to_path(
        &mut self,
        path: PathBuf,
        project: Project,
    ) -> Result<(), SessionError> {
        Project::save_project(&path, &project)?;
        dep_mut!(AutoPersisting<Config>, |config| {
            let _ = config.modify(ConfigModification::AddRecentProject(path.clone()));
            let _ = config.modify(ConfigModification::SetLastProject(path.clone()));
        });

        self.saved_project = Some(project);
        self.active_project = Some(path);
        Ok(())
    }

    pub fn mark_project_loaded(&mut self, path: PathBuf, project: Project) {
        self.active_project = Some(path);
        self.project_preferences = project.preferences.clone();
        self.saved_project = Some(project);
    }

    pub fn new_project(
        &mut self,
        current_scene: &OrganizeEditScene,
    ) -> Result<OrganizeEditScene, SessionError> {
        if self.file_dialog_open {
            return Err(SessionError::WaitingForUserInput);
        }

        if self.has_unsaved_changes(current_scene) {
            self.pending_operation = Some(PendingOperation::NewProject);
            self.save_warning_modal_id = Some(ModalManager::push(SaveWarningModal::new(
                SaveWarningSource::NewProject,
            )));
            return Err(SessionError::WaitingForUserInput);
        }

        self.new_project_internal()
    }

    fn execute_pending_operation(&mut self, ctx: &Context) -> Option<OrganizeEditScene> {
        let operation = self.pending_operation.take()?;
        self.execute_operation(operation, ctx)
    }

    fn execute_operation(
        &mut self,
        operation: PendingOperation,
        ctx: &Context,
    ) -> Option<OrganizeEditScene> {
        match operation {
            PendingOperation::NewProject => match self.new_project_internal() {
                Ok(scene) => Some(scene),
                Err(e) => {
                    log::error!("Error creating new project: {:?}", e);
                    None
                }
            },
            PendingOperation::LoadProject(path) => match self.load_project_internal(path, ctx) {
                Ok(scene) => Some(scene),
                Err(SessionError::WaitingForUserInput) => None,
                Err(e) => {
                    log::error!("Error loading project: {:?}", e);
                    None
                }
            },
        }
    }

    fn load_project_internal(
        &mut self,
        path: Option<PathBuf>,
        ctx: &Context,
    ) -> Result<OrganizeEditScene, SessionError> {
        let path = match path {
            Some(p) => p,
            None => {
                let dialog = native_dialog::DialogBuilder::file()
                    .add_filter("Photo Book Collection", ["rpb"])
                    .open_single_file()
                    .spawn();
                self.file_dialog_open = true;
                file_dialog::spawn(dialog, ctx.clone(), |result, _| {
                    dep_mut!(Session, |session| session.finish_open_dialog(result));
                });
                return Err(SessionError::WaitingForUserInput);
            }
        };
        self.load_project_from_path(path)
    }

    fn load_project_from_path(&mut self, path: PathBuf) -> Result<OrganizeEditScene, SessionError> {
        dep_mut!(PhotoManager, |photo_manager| {
            photo_manager.clear();
        });
        dep_mut!(SelectionManager, |selection_manager| selection_manager
            .clear());

        let project = Project::load_project(&path)?;
        self.project_preferences = project.preferences.clone();
        let mut scene: OrganizeEditScene = project.clone().into();
        scene.apply_project_preferences(&self.project_preferences);

        dep_mut!(AutoPersisting<Config>, |config| {
            let _ = config.modify(ConfigModification::AddRecentProject(path.clone()));
            let _ = config.modify(ConfigModification::SetLastProject(path.clone()));
        });

        self.mark_project_loaded(path, project);

        Ok(scene)
    }

    fn finish_save_dialog(
        &mut self,
        result: FileDialogResult,
        project: Project,
        continuation: Option<PendingOperation>,
        ctx: &Context,
    ) {
        self.file_dialog_open = false;
        if let Some(modal_id) = self.save_warning_modal_id {
            self.dismiss_save_warning(modal_id);
        }

        let path = match result {
            Ok(Some(path)) => path,
            Ok(None) => return,
            Err(error) => {
                error!("Error opening save file dialog: {error}");
                return;
            }
        };

        if let Err(error) = self.save_project_to_path(path, project) {
            error!("Error saving collection: {error:?}");
            return;
        }

        if let Some(operation) = continuation {
            self.completed_scene = self.execute_operation(operation, ctx);
        }
    }

    fn finish_open_dialog(&mut self, result: FileDialogResult) {
        self.file_dialog_open = false;

        let path = match result {
            Ok(Some(path)) => path,
            Ok(None) => return,
            Err(error) => {
                error!("Error opening collection file dialog: {error}");
                return;
            }
        };

        match self.load_project_from_path(path) {
            Ok(scene) => self.completed_scene = Some(scene),
            Err(error) => error!("Error loading collection: {error:?}"),
        }
    }

    fn dismiss_save_warning(&mut self, modal_id: TypedModalId<SaveWarningModal>) {
        dep_mut!(ModalManager, |modal_manager| modal_manager
            .dismiss(modal_id));
        self.save_warning_modal_id = None;
    }

    fn new_project_internal(&mut self) -> Result<OrganizeEditScene, SessionError> {
        self.active_project = None;
        self.project_preferences = ProjectPreferences::default();
        self.saved_project = None;
        let mut scene = OrganizeEditScene::new(GalleryScene::new(), None);
        scene.apply_project_preferences(&self.project_preferences);

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

        let current_project =
            Project::new_with_preferences(current_scene, self.project_preferences.clone());
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
