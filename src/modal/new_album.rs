use crate::{
    dependencies::{Dependency, SingletonFor},
    photo_manager::PhotoManager,
};

use super::{Modal, ModalActionResponse};

#[derive(Debug, Clone, Default)]
pub struct NewAlbumModal {
    album_name: String,
    submit_requested: bool,
}

impl NewAlbumModal {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Modal for NewAlbumModal {
    type Response = ModalActionResponse;

    fn title(&self) -> String {
        "New Album".to_string()
    }

    fn body_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Album name");
        let response = ui.text_edit_singleline(&mut self.album_name);
        if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
            self.submit_requested = true;
        }
    }

    fn actions_ui(&mut self, ui: &mut egui::Ui) -> Option<Self::Response> {
        if ui.button("Cancel").clicked() {
            return Some(ModalActionResponse::Cancel);
        }

        let can_create = !self.album_name.trim().is_empty();
        let create_clicked = ui
            .add_enabled(can_create, egui::Button::new("Create"))
            .clicked();

        if (create_clicked || self.submit_requested) && can_create {
            let album_name = self.album_name.trim().to_string();
            Dependency::<PhotoManager>::get()
                .with_lock_mut(|photo_manager| photo_manager.create_album(&album_name));

            return Some(ModalActionResponse::Confirm);
        }

        self.submit_requested = false;

        None
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
