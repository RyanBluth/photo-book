use super::{Modal, ModalActionResponse};

#[derive(Debug, Clone, Default)]
pub struct SettingsModal;

impl SettingsModal {
    pub fn new() -> Self {
        Self
    }
}

impl Modal for SettingsModal {
    type Response = ModalActionResponse;

    fn title(&self) -> String {
        "Settings".to_string()
    }

    fn body_ui(&mut self, _ui: &mut egui::Ui) {}

    fn actions_ui(&mut self, _ui: &mut egui::Ui) -> Option<Self::Response> {
        None
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
