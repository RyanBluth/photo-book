use super::{Modal, ModalActionResponse};
pub struct BasicModal {
    title: String,
    message: String,
    dismiss_title: String,
}

impl BasicModal {
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        dismiss_title: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            dismiss_title: dismiss_title.into(),
        }
    }
}

impl Modal for BasicModal {
    fn title(&self) -> String {
        self.title.clone()
    }

    fn body_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(&self.message);
    }

    fn actions_ui(&mut self, ui: &mut egui::Ui) -> ModalActionResponse {
        if ui.button(&self.dismiss_title).clicked() {
            return ModalActionResponse::Cancel;
        }
        ModalActionResponse::None
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
