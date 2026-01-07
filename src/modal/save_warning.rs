use std::{any::Any, path::PathBuf};

use crate::modal::{Modal, ModalResponse};

#[derive(Debug, Clone)]
pub enum SaveWarningSource {
    NewProject,
    #[allow(dead_code)]
    LoadProject(Option<PathBuf>),
}

pub struct SaveWarningModal {
    pub source: SaveWarningSource,
}

#[derive(Debug, Clone, Copy)]
pub enum SaveWarningResponse {
    Save,
    DontSave,
    Cancel,
}

impl ModalResponse for SaveWarningResponse {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn should_close(&self) -> bool {
        true
    }
}

impl SaveWarningModal {
    pub fn new(source: SaveWarningSource) -> Self {
        Self { source }
    }
}

impl Modal for SaveWarningModal {
    type Response = SaveWarningResponse;

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

    fn actions_ui(&mut self, ui: &mut egui::Ui) -> Option<Self::Response> {
        if ui.button("Save").clicked() {
            Some(SaveWarningResponse::Save)
        } else if ui.button("Don't Save").clicked() {
            Some(SaveWarningResponse::DontSave)
        } else if ui.button("Cancel").clicked() {
            Some(SaveWarningResponse::Cancel)
        } else {
            None
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
