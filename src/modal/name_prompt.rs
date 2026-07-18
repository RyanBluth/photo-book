use std::any::Any;

use crate::theme::style;

use super::{Modal, ModalResponse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamePromptResponse {
    Cancel,
    Confirm { name: String },
}

impl ModalResponse for NamePromptResponse {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn should_close(&self) -> bool {
        true
    }

    fn cancel() -> Option<Self> {
        Some(Self::Cancel)
    }
}

#[derive(Debug, Clone)]
pub struct NamePromptModal {
    title: String,
    label: String,
    confirm_label: String,
    name: String,
    submit_requested: bool,
}

impl NamePromptModal {
    pub fn new(
        title: impl Into<String>,
        label: impl Into<String>,
        confirm_label: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            label: label.into(),
            confirm_label: confirm_label.into(),
            name: String::new(),
            submit_requested: false,
        }
    }
}

impl Modal for NamePromptModal {
    type Response = NamePromptResponse;

    fn title(&self) -> String {
        self.title.clone()
    }

    fn body_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(&self.label);
        let response = ui.add_sized(
            [ui.available_width(), ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut self.name),
        );
        if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
            self.submit_requested = true;
        }
    }

    fn actions_ui(&mut self, ui: &mut egui::Ui) -> Option<Self::Response> {
        let name = self.name.trim();
        let can_confirm = !name.is_empty();
        let confirm_clicked =
            style::primary_button_enabled(ui, can_confirm, &self.confirm_label).clicked();

        if (confirm_clicked || self.submit_requested) && can_confirm {
            return Some(NamePromptResponse::Confirm {
                name: name.to_string(),
            });
        }

        if ui.button("Cancel").clicked() {
            return Some(NamePromptResponse::Cancel);
        }

        self.submit_requested = false;

        None
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
