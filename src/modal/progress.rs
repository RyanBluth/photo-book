use super::{Modal, ModalActionResponse};
use crate::cancellation::CancellationToken;

pub struct ProgressModal {
    pub message: String,
    pub progress: f32,

    dismiss_title: String,
    title: String,
    cancellation: Option<CancellationToken>,
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
            cancellation: None,
        }
    }

    pub fn with_cancellation(mut self, cancellation: CancellationToken) -> Self {
        self.cancellation = Some(cancellation);
        self
    }
}

impl Modal for ProgressModal {
    type Response = ModalActionResponse;

    fn title(&self) -> String {
        self.title.clone()
    }

    fn body_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(&self.message);
        ui.add(egui::ProgressBar::new(self.progress).animate(true));
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn actions_ui(&mut self, ui: &mut egui::Ui) -> Option<Self::Response> {
        if ui.button(&self.dismiss_title).clicked() {
            if let Some(cancellation) = &self.cancellation {
                cancellation.cancel();
            }
            return Some(ModalActionResponse::Cancel);
        }

        None
    }
}
