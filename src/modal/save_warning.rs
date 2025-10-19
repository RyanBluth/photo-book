use std::path::{Path, PathBuf};

use crate::modal::{Modal, ModalActionResponse};

#[derive(Debug, Clone)]
pub enum SaveWarningSource {
    NewProject,
    LoadProject(Option<PathBuf>),
}

pub struct SaveWarningModal {
    pub source: SaveWarningSource,
}

impl SaveWarningModal {
    pub fn new(source: SaveWarningSource) -> Self {
        Self { source }
    }
}

impl Modal for SaveWarningModal {
    fn title(&self) -> String {
        "Unsaved Changes".to_string()
    }

    fn body_ui(&mut self, ui: &mut egui::Ui) {
        let message = match &self.source {
            SaveWarningSource::NewProject => {
                "You have unsaved changes. Would you like to save before creating a new project?"
            }
            SaveWarningSource::LoadProject(_) => {
                "You have unsaved changes. Would you like to save before loading a different project?"
            }
        };
        ui.label(message);
    }

    fn actions_ui(&mut self, ui: &mut egui::Ui) -> ModalActionResponse {
        if ui.button("Save").clicked() {
            ModalActionResponse::Confirm
        } else if ui.button("Don't Save").clicked() {
            ModalActionResponse::Cancel
        } else if ui.button("Cancel").clicked() {
            ModalActionResponse::Close
        } else {
            ModalActionResponse::None
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
