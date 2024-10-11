use std::sync::Mutex;

use egui::{Button, Color32, Layout, Vec2};
use indexmap::IndexMap;

use crate::{dependencies::{Dependency, Singleton, SingletonFor}, id::{next_modal_id, ModalId}};

use super::{ModalResponse, Modal};



pub struct ModalManager {
    modals: IndexMap<ModalId, Mutex<Box<dyn Modal>>>,
}

impl ModalManager {
    pub fn new() -> Self {
        Self {
            modals: IndexMap::new(),
        }
    }

    pub fn push<T: Modal + Send + 'static>(modal: T) -> ModalId {
        let modal_manager: Singleton<ModalManager> = Dependency::get();
        modal_manager.with_lock_mut(|modal_manager| {
            let id = next_modal_id();
            let boxed = Box::new(modal);
            modal_manager.modals.insert(id, Mutex::new(boxed));
            id
        })
    }

    pub fn dismiss(&mut self, id: ModalId) {
        self.modals.shift_remove(&id);
    }

    pub fn show_next(&mut self, ui: &mut egui::Ui) {
        match self.modals.keys().last() {
            Some(id) => {
                let response = self.show_modal(ui, *id);

                match response {
                    ModalResponse::Dismiss => {
                        self.modals.pop();
                    }
                    _ => {}
                }
            }
            None => {}
        }
    }

    fn show_modal(&self, ui: &mut egui::Ui, modal_id: ModalId) -> ModalResponse {
        let mut_modal = self.modals.get(&modal_id);
        match mut_modal {
            Some(modal) => {
                let mut modal = modal.lock().unwrap();
                let viewport_rect = ui
                    .ctx()
                    .viewport(|viewport| viewport.this_pass.available_rect);

                ui.painter()
                    .rect_filled(viewport_rect, 0.0, Color32::from_black_alpha(128));

                let mut response = ModalResponse::None;

                egui::Window::new(&modal.title())
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    .resizable(false)
                    .collapsible(false)
                    .min_size(Vec2::new(400.0, 300.0))
                    .show(ui.ctx(), |ui: &mut egui::Ui| {
                        modal.ui(ui);

                        ui.add_space(20.0);
                        ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                            if ui.button(&modal.dismiss_label()).clicked() {
                                response = ModalResponse::Dismiss;
                            }

                            if let Some(actions) = modal.actions().as_ref() {
                                actions.iter().for_each(|action| {
                                    if ui
                                        .add_enabled(action.is_enabled, Button::new(&action.label))
                                        .clicked()
                                    {
                                        (*action.func.lock().unwrap())();
                                        response = ModalResponse::Dismiss;
                                    }
                                });
                            }
                        });
                    });

                response
            }
            None => ModalResponse::None,
        }
    }
}