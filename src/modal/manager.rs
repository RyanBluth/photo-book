use std::{collections::HashMap, sync::Mutex};

use egui::{Color32, Layout, Vec2};
use indexmap::IndexMap;

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    id::{next_modal_id, ModalId},
};

use super::{Modal, ModalActionResponse};

#[derive(Debug, Clone)]
pub struct TypedModalId<T> {
    id: ModalId,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> Into<ModalId> for TypedModalId<T> {
    fn into(self) -> ModalId {
        self.id
    }
}

impl<T> Into<ModalId> for &TypedModalId<T> {
    fn into(self) -> ModalId {
        self.id
    }
}

pub struct ModalManager {
    modals: IndexMap<ModalId, Mutex<Box<dyn Modal>>>,
    responses: HashMap<ModalId, ModalActionResponse>
}

impl ModalManager {
    pub fn new() -> Self {
        Self {
            modals: IndexMap::new(),
            responses: HashMap::new()
        }
    }

    pub fn modify<T: Modal + 'static>(&self, id: &TypedModalId<T>, f: impl FnOnce(&mut T)) -> bool {
        if let Some(mutex) = self.modals.get(&id.id) {
            if let Ok(mut guard) = mutex.lock() {
                if let Some(modal) = guard.as_any_mut().downcast_mut::<T>() {
                    f(modal);
                    return true;
                }
            }
        }
        false
    }

    pub fn push<T: Modal + Send + 'static>(modal: T) -> TypedModalId<T> {
        let modal_manager: Singleton<ModalManager> = Dependency::get();
        let id = modal_manager.with_lock_mut(|modal_manager| {
            let id = next_modal_id();
            let boxed = Box::new(modal);
            modal_manager.modals.insert(id, Mutex::new(boxed));
            id
        });
        TypedModalId {
            id,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn response_for(&self, id: impl Into<ModalId>) -> Option<ModalActionResponse> {
        self.responses.get(&id.into()).copied()
    }

    pub fn dismiss(&mut self, id: impl Into<ModalId>) {
        self.modals.shift_remove(&id.into());
    }

    pub fn exists(&self, id: impl Into<ModalId>) -> bool {
        self.modals.contains_key(&id.into())
    }

    pub fn show_next(&mut self, ui: &mut egui::Ui) {
        self.responses.clear();

        match self.modals.keys().last() {
            Some(id) => {
                let response = self.show_modal(ui, *id);

                match response {
                    ModalActionResponse::Cancel | ModalActionResponse::Confirm => {
                        self.modals.pop();
                    }
                    _ => {}
                }
            }
            None => {}
        }
    }

    fn show_modal(&mut self, ui: &mut egui::Ui, modal_id: ModalId) -> ModalActionResponse {
        let mut_modal = self.modals.get(&modal_id);
        match mut_modal {
            Some(modal) => {
                let mut modal = modal.lock().unwrap();
                let viewport_rect = ui
                    .ctx()
                    .viewport(|viewport| viewport.this_pass.available_rect);

                ui.painter()
                    .rect_filled(viewport_rect, 0.0, Color32::from_black_alpha(128));

                let mut response = ModalActionResponse::None;

                egui::Window::new(&modal.title())
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    .resizable(false)
                    .collapsible(false)
                    .min_size(Vec2::new(400.0, 300.0))
                    .show(ui.ctx(), |ui: &mut egui::Ui| {
                        modal.body_ui(ui);
                        ui.add_space(20.0);
                        ui.with_layout(Layout::left_to_right(egui::Align::Min), |ui| {
                            response = modal.actions_ui(ui);
                        });
                    });

                self.responses.insert(modal_id, response);

                response
            }
            None => ModalActionResponse::None,
        }
    }
}
