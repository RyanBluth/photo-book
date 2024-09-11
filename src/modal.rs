use std::sync::RwLockWriteGuard;

use egui::{Color32, Response, Vec2, Widget};

use crate::dependencies::{Dependency, Singleton, SingletonFor};

pub enum ModalContent {
    Message(String),
}

pub enum ModalResponse {
    Dismiss,
    None,
}

pub struct ModalState {
    pub content: ModalContent,
}

pub struct Modal {
    pub state: ModalState,
}

impl Modal {
    pub fn new(content: ModalContent) -> Self {
        Self {
            state: ModalState { content },
        }
    }

    pub fn show(&self, ui: &mut egui::Ui) -> ModalResponse {
        let viewport_rect = ui
            .ctx()
            .viewport(|viewport| viewport.this_frame.available_rect);

        ui.painter()
            .rect_filled(viewport_rect, 0.0, Color32::from_black_alpha(128));

        egui::Window::new("Modal")
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .resizable(false)
            .collapsible(false)
            .min_size(Vec2::new(400.0,300.0))
            .show(ui.ctx(), |ui| match &self.state.content {
                ModalContent::Message(message) => {
                    ui.label(message);
                }
            });

        ModalResponse::None
    }
}

pub struct ModalManager {
    modals: Vec<Modal>,
}

impl ModalManager {
    pub fn new() -> Self {
        Self { modals: Vec::new() }
    }

    pub fn push(&mut self, modal: Modal) {
        self.modals.push(modal);
    }

    pub fn push_modal(content: ModalContent) {
        let modal_manager: Singleton<ModalManager> = Dependency::get();
        modal_manager.with_lock_mut(|modals| {
            modals.push(Modal::new(content));
        });
    }

    pub fn show_next(&mut self, ui: &mut egui::Ui) {
        let modal_response = self
            .modals
            .last()
            .as_ref()
            .and_then(|modal| Some(modal.show(ui)));

        match modal_response {
            Some(ModalResponse::Dismiss) => {
                self.modals.pop();
            }
            _ => {}
        }
    }
}
