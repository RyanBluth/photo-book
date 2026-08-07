use crate::{dep_mut, model::album, photo_manager::PhotoManager};
use std::path::PathBuf;

use super::{
    Modal, ModalActionResponse,
    name_prompt::{NamePromptModal, NamePromptResponse},
};

#[derive(Debug, Clone)]
pub struct NewAlbumModal {
    prompt: NamePromptModal,
    photos: Option<Vec<PathBuf>>,
}

impl NewAlbumModal {
    pub fn new() -> Self {
        Self {
            prompt: NamePromptModal::new("New Album", "Album name", "Create"),
            photos: None,
        }
    }

    pub fn with_photos(photos: Vec<PathBuf>) -> Self {
        Self {
            prompt: NamePromptModal::new("New Album", "Album name", "Create"),
            photos: Some(photos),
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
                let album_id = dep_mut!(PhotoManager, |photo_manager| photo_manager
                    .create_album(&name));

                if let Some(photos) = &self.photos
                    && let Some(album_id) = album_id
                {
                    dep_mut!(PhotoManager, |photo_manager| {
                        for photo_path in photos {
                            photo_manager.add_to_album(&album_id, photo_path);
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
