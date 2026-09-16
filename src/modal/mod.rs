use std::any::Any;

pub mod basic;
pub mod file_import;
pub mod manager;
pub mod name_prompt;
pub mod new_album;
pub mod new_tag;
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

    /// The response to emit when egui dismisses the modal via its backdrop or Escape key.
    fn cancel() -> Option<Self>
    where
        Self: Sized,
    {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModalActionResponse {
    Cancel,
    Confirm,
    _Close,
}

impl ModalResponse for ModalActionResponse {
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
