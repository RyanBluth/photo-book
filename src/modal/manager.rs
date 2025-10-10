use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use egui::{Color32, Layout, Vec2};
use indexmap::IndexMap;

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    id::{ModalId, next_modal_id},
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

/// Manages modal dialogs in the application
///
/// The modal manager keeps track of active modals and their responses.
/// It supports typed modal IDs for type-safe modal management and
/// provides methods for showing, dismissing and modifying modals.
///
/// # Example
/// ```
/// let modal_id = ModalManager::push(MyModal::new());
///
/// // Later modify the modal
/// modal_manager.modify(&modal_id, |modal| {
///     modal.update_value(42);
/// });
/// ```

pub struct ModalManager {
    modals: IndexMap<ModalId, Arc<Mutex<Box<dyn Modal>>>>,
    responses: HashMap<ModalId, ModalActionResponse>,
}

#[derive(Debug, thiserror::Error)]
pub enum ModalError {
    #[error("Modal not found with ID {0}")]
    NotFound(ModalId),
    #[error("Failed to lock modal mutex")]
    LockError,
    #[error("Modal type mismatch")]
    TypeMismatch,
}

impl ModalManager {
    pub fn new() -> Self {
        Self {
            modals: IndexMap::new(),
            responses: HashMap::new(),
        }
    }

    pub fn modify<T: Modal + 'static>(
        &self,
        id: &TypedModalId<T>,
        f: impl FnOnce(&mut T),
    ) -> Result<(), ModalError> {
        let mutex = self.modals.get(&id.id).ok_or(ModalError::NotFound(id.id))?;

        let mut guard = mutex.lock().map_err(|_| ModalError::LockError)?;

        let modal = guard
            .as_any_mut()
            .downcast_mut::<T>()
            .ok_or(ModalError::TypeMismatch)?;

        f(modal);
        Ok(())
    }

    pub fn push<T: Modal + Send + 'static>(modal: T) -> TypedModalId<T> {
        let modal_manager: Singleton<ModalManager> = Dependency::get();
        let id = modal_manager.with_lock_mut(|modal_manager| {
            let id = next_modal_id();
            let boxed: Box<dyn Modal> = Box::new(modal);
            modal_manager.modals.insert(id, Arc::new(Mutex::new(boxed)));
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
        self.responses.values().for_each(|response| match response {
            ModalActionResponse::Cancel | ModalActionResponse::Confirm => {
                self.modals.pop();
            }
            _ => {}
        });
        self.responses.clear();

        match self.modals.keys().last() {
            Some(id) => {
                _ = self.show_modal(ui, *id);
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
