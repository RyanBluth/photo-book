use std::sync::{Arc, Mutex, RwLockWriteGuard};

use egui::{
    Align::{Center, Min},
    Color32, Layout, Response, Vec2, Widget,
};

use crate::dependencies::{Dependency, Singleton, SingletonFor};

pub enum ModalContent {
    Message(String),
}

pub struct ModalAction {
    pub func: Mutex<Box<dyn Fn() -> () + Send>>,
    pub label: String,
}

pub enum ModalResponse {
    Dismiss,
    None,
}

pub struct ModalState {
    pub title: String,
    pub content: ModalContent,
    pub dismiss_label: String,
    pub actions: Option<Vec<ModalAction>>,
}

pub struct Modal {
    pub state: ModalState,
}

impl Modal {
    pub fn new(
        title: String,
        content: ModalContent,
        dismiss_label: String,
        actions: Option<Vec<ModalAction>>,
    ) -> Self {
        Self {
            state: ModalState {
                title,
                content,
                dismiss_label,
                actions,
            },
        }
    }
    pub fn show(&self, ui: &mut egui::Ui) -> ModalResponse {
        let viewport_rect = ui
            .ctx()
            .viewport(|viewport| viewport.this_frame.available_rect);

        ui.painter()
            .rect_filled(viewport_rect, 0.0, Color32::from_black_alpha(128));

        let mut response = ModalResponse::None;

        egui::Window::new(&self.state.title)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .resizable(false)
            .collapsible(false)
            .min_size(Vec2::new(400.0, 300.0))
            .show(ui.ctx(), |ui| {
                match &self.state.content {
                    ModalContent::Message(message) => {
                        ui.label(message);
                    }
                }

                ui.add_space(20.0);
                ui.with_layout(Layout::right_to_left(Min), |ui| {
                    if ui.button(&self.state.dismiss_label).clicked() {
                        response = ModalResponse::Dismiss;
                    }

                    if let Some(actions) = self.state.actions.as_ref() {
                        actions.iter().for_each(|action| {
                            if ui.button(&action.label).clicked() {
                                (*action.func.lock().unwrap())();
                                response = ModalResponse::Dismiss;
                            }
                        });
                    }
                });
            });

        response
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

    pub fn push_basic_modal(title: impl Into<String>, message: impl Into<String>) {
        let modal_manager: Singleton<ModalManager> = Dependency::get();
        modal_manager.with_lock_mut(|modals| {
            modals.push(Modal::new(
                title.into(),
                ModalContent::Message(message.into()),
                "OK".to_string(),
                None,
            ));
        });
    }

    pub fn push_modal(
        title: impl Into<String>,
        content: ModalContent,
        dismiss_label: impl Into<String>,
        actions: Option<Vec<ModalAction>>,
    ) {
        let modal_manager: Singleton<ModalManager> = Dependency::get();
        modal_manager.with_lock_mut(|modals| {
            modals.push(Modal::new(
                title.into(),
                content,
                dismiss_label.into(),
                actions,
            ));
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
