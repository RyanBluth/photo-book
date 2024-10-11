use super::{ModalAction, ModalResponse, Modal};


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

    fn ui(&mut self, ui: &mut egui::Ui) -> ModalResponse {
        ui.label(&self.message);

        ModalResponse::None
    }

    fn dismiss_label(&self) -> String {
        self.dismiss_title.clone()
    }

    fn actions(&self) -> Option<Vec<ModalAction>> {
        None
    }
}