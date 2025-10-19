use std::any::Any;

pub mod basic;
pub mod custom;
pub mod manager;
pub mod page_settings;
pub mod photo_filter;
pub mod progress;
pub mod save_warning;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModalActionResponse {
    Cancel,
    Confirm,
    Close,
    None,
}

pub trait Modal: Send + Any {
    fn title(&self) -> String;

    fn body_ui(&mut self, ui: &mut egui::Ui);

    fn actions_ui(&mut self, ui: &mut egui::Ui) -> ModalActionResponse;

    fn as_any_mut(&mut self) -> &mut dyn Any;
}
