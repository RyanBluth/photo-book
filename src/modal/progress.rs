use super::{Modal, ModalAction, ModalResponse};

pub struct ProgressModal {
    pub message: String,
    pub progress: f32,

    dismiss_title: String,
    title: String,
}

impl ProgressModal {
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        dismiss_title: impl Into<String>,
        progress: f32,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            dismiss_title: dismiss_title.into(),
            progress,
        }
    }
}

impl Modal for ProgressModal {
    fn title(&self) -> String {
        self.title.clone()
    }

    fn ui(&mut self, ui: &mut egui::Ui) -> ModalResponse {
        ui.label(&self.message);
        ui.add(egui::ProgressBar::new(self.progress).animate(true));
        ModalResponse::None
    }

    fn dismiss_label(&self) -> String {
        self.dismiss_title.clone()
    }

    fn actions(&self) -> Option<Vec<ModalAction>> {
        None
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
