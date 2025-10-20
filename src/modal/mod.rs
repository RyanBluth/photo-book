use std::any::Any;

pub mod basic;
pub mod manager;
pub mod page_settings;
pub mod photo_filter;
pub mod progress;
pub mod save_warning;

pub trait Modal: Send + Any {
    type Response: ModalResponse + 'static;

    fn title(&self) -> String;
    fn body_ui(&mut self, ui: &mut egui::Ui);
    fn actions_ui(&mut self, ui: &mut egui::Ui) -> Option<Self::Response>;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub trait ModalResponse: Send + Any {
    fn as_any(&self) -> &dyn Any;
    fn should_close(&self) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModalActionResponse {
    Cancel,
    Confirm,
    Close,
}

impl ModalResponse for ModalActionResponse {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn should_close(&self) -> bool {
        true
    }
}
