use crate::{dep_mut, photo_manager::PhotoManager};
use std::path::PathBuf;

use super::{
    Modal, ModalActionResponse,
    name_prompt::{NamePromptModal, NamePromptResponse},
};

#[derive(Debug, Clone)]
pub struct NewTagModal {
    prompt: NamePromptModal,
    photos: Option<Vec<PathBuf>>,
}

impl NewTagModal {
    pub fn with_photos(photos: Vec<PathBuf>) -> Self {
        Self {
            prompt: NamePromptModal::new("New Tag", "Tag name", "Create"),
            photos: Some(photos),
        }
    }
}

impl Modal for NewTagModal {
    type Response = ModalActionResponse;

    fn title(&self) -> String {
        self.prompt.title()
    }

    fn body_ui(&mut self, ui: &mut egui::Ui) {
        self.prompt.body_ui(ui);
    }

    fn actions_ui(&mut self, ui: &mut egui::Ui) -> Option<Self::Response> {
        match self.prompt.actions_ui(ui) {
            Some(NamePromptResponse::Confirm { name }) => {
                if let Some(photos) = &self.photos {
                    dep_mut!(PhotoManager, |photo_manager| {
                        for photo_path in photos {
                            photo_manager.add_photo_tag(photo_path, name.clone());
                        }
                    });
                }

                Some(ModalActionResponse::Confirm)
            }
            Some(NamePromptResponse::Cancel) => Some(ModalActionResponse::Cancel),
            None => None,
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
