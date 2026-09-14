mod graph;
mod graph_model;
mod node_editor;
mod preview;
mod preview_tree;
mod saved_workflows;
mod ui;
pub(crate) mod workflow;

#[cfg(test)]
mod tests;

use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::{
    dep, dep_mut,
    modal::{
        Modal, ModalActionResponse, basic::BasicModal, manager::ModalManager,
        progress::ProgressModal,
    },
    photo::Photo,
    photo_manager::PhotoManager,
    theme::style,
};

use self::{
    graph::WorkflowGraphState,
    preview::ImportPreviewState,
    workflow::{FileImportWorkflow, FileWorkflow},
};

#[allow(unused_imports)]
pub use workflow::FileImportWorkflowError;

pub struct FileImportModal {
    source_path: Option<PathBuf>,
    destination_path: Option<PathBuf>,
    add_to_collection: bool,
    workflow: FileWorkflow,
    graph: WorkflowGraphState,
    preview_state: Arc<Mutex<ImportPreviewState>>,
    preview_expanded: HashSet<PathBuf>,
    saved_workflows: saved_workflows::SavedWorkflowState,
}

impl FileImportModal {
    pub fn new() -> Self {
        Self {
            source_path: None,
            destination_path: None,
            add_to_collection: true,
            workflow: FileWorkflow::default(),
            graph: WorkflowGraphState::default(),
            preview_state: Arc::new(Mutex::new(ImportPreviewState::default())),
            preview_expanded: HashSet::new(),
            saved_workflows: saved_workflows::SavedWorkflowState::default(),
        }
    }

    fn start_import(&self, ctx: &egui::Context) {
        let workflow = FileImportWorkflow {
            source_path: self.source_path.clone(),
            destination_path: self.destination_path.clone(),
            workflow: self.workflow.clone(),
        };
        let add_to_collection = self.add_to_collection;
        let ctx = ctx.clone();

        tokio::task::spawn_blocking(move || {
            let progress_modal_id = ModalManager::push(ProgressModal::new(
                "Importing",
                "Copying files…",
                "Run in Background",
                0.0,
            ));
            ctx.request_repaint();

            let result = workflow.run();
            dep_mut!(ModalManager, |modal_manager| {
                modal_manager.dismiss(progress_modal_id);
            });

            match result {
                Ok(imported_paths) => {
                    let mut failed_images = 0;
                    if add_to_collection {
                        let (photos, failures) = Self::imported_photos(imported_paths);
                        failed_images = failures;
                        if !photos.is_empty() {
                            dep!(PhotoManager, |manager| manager.load_photos(photos));
                        }
                    }
                    let message = if failed_images == 0 {
                        "The files were imported successfully.".to_owned()
                    } else {
                        format!(
                            "The files were copied successfully, but {failed_images} images could not be added to the collection."
                        )
                    };
                    ModalManager::push(BasicModal::new("Import Complete", message, "Close"));
                }
                Err(error) => {
                    ModalManager::push(BasicModal::new(
                        "Import Failed",
                        error.to_string(),
                        "Close",
                    ));
                }
            }
            ctx.request_repaint();
        });
    }

    fn imported_photos(paths: Vec<PathBuf>) -> (Vec<Photo>, usize) {
        let mut photos = Vec::new();
        let mut failures = 0;
        for path in paths {
            // Match the formats supported by the collection's folder loader.
            let extension = path
                .extension()
                .map(|extension| extension.to_ascii_lowercase());
            if !extension.is_some_and(|extension| {
                extension == "jpg" || extension == "jpeg" || extension == "png"
            }) {
                continue;
            }
            match Photo::new(path.clone()) {
                Ok(photo) => photos.push(photo),
                Err(error) => {
                    log::error!("Could not add imported image {}: {error}", path.display());
                    failures += 1;
                }
            }
        }
        (photos, failures)
    }
}

impl Modal for FileImportModal {
    type Response = ModalActionResponse;

    fn title(&self) -> String {
        "Import".to_string()
    }

    fn body_ui(&mut self, ui: &mut egui::Ui) {
        self.import_body_ui(ui);
    }

    fn actions_ui(&mut self, ui: &mut egui::Ui) -> Option<Self::Response> {
        let can_import = self.source_path.is_some() && self.destination_path.is_some();
        if style::primary_button_enabled(ui, can_import, "Import").clicked() {
            self.start_import(ui.ctx());

            return Some(ModalActionResponse::Confirm);
        }

        if ui.button("Cancel").clicked() {
            return Some(ModalActionResponse::Cancel);
        }
        None
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
