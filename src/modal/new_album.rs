use crate::{dep_mut, photo_manager::PhotoManager};

use super::{
    Modal, ModalActionResponse,
    name_prompt::{NamePromptModal, NamePromptResponse},
};

#[derive(Debug, Clone)]
pub struct NewAlbumModal {
    prompt: NamePromptModal,
}

impl NewAlbumModal {
    pub fn new() -> Self {
        Self {
            prompt: NamePromptModal::new("New Album", "Album name", "Create"),
        }
    }
}

impl Modal for NewAlbumModal {
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
                dep_mut!(PhotoManager, |photo_manager| photo_manager
                    .create_album(&name));

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
