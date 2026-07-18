use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use indexmap::IndexMap;

use crate::{
    dep_mut,
    id::{ModalId, next_modal_id},
    modal::ModalResponse,
    theme::{color, style},
};

use super::Modal;

#[derive(Debug)]
pub struct TypedModalId<T: Modal> {
    id: ModalId,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Modal> Clone for TypedModalId<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Modal> Copy for TypedModalId<T> {}

impl<T: Modal> From<TypedModalId<T>> for ModalId {
    fn from(val: TypedModalId<T>) -> Self {
        val.id
    }
}

impl<T: Modal> From<&TypedModalId<T>> for ModalId {
    fn from(val: &TypedModalId<T>) -> Self {
        val.id
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
    modals: IndexMap<ModalId, Arc<Mutex<Box<dyn DynModal>>>>,
    responses: HashMap<ModalId, Mutex<Box<dyn ModalResponse>>>,
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

    pub fn _read<T: Modal + 'static, R>(
        &self,
        id: &TypedModalId<T>,
        f: impl FnOnce(&T) -> R,
    ) -> Result<R, ModalError> {
        let mutex = self.modals.get(&id.id).ok_or(ModalError::NotFound(id.id))?;

        let mut guard = mutex.lock().map_err(|_| ModalError::LockError)?;

        let modal = guard
            .as_any_mut()
            .downcast_mut::<T>()
            .ok_or(ModalError::TypeMismatch)?;

        Ok(f(modal))
    }

    pub fn push<T: Modal + Send + 'static>(modal: T) -> TypedModalId<T> {
        let id = dep_mut!(ModalManager, |modal_manager| {
            let id = next_modal_id();
            let boxed: Box<dyn DynModal> = Box::new(modal);
            modal_manager.modals.insert(id, Arc::new(Mutex::new(boxed)));
            id
        });

        TypedModalId {
            id,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn response_for<T: Modal>(
        &self,
        id: &TypedModalId<T>,
    ) -> Result<Option<T::Response>, ModalError>
    where
        T::Response: Clone,
    {
        match self.responses.get(&id.into()) {
            Some(response) => {
                let guard = response.lock().unwrap();
                let response = guard
                    .as_any()
                    .downcast_ref::<T::Response>()
                    .ok_or(ModalError::TypeMismatch)?;
                Ok(Some(response.clone()))
            }
            None => Ok(None),
        }
    }

    pub fn dismiss(&mut self, id: impl Into<ModalId>) {
        self.modals.shift_remove(&id.into());
    }

    pub fn exists(&self, id: impl Into<ModalId>) -> bool {
        self.modals.contains_key(&id.into())
    }

    pub fn show_next(&mut self, ui: &mut egui::Ui) {
        let modal_ids_to_close = self
            .responses
            .iter()
            .filter_map(|(modal_id, response)| {
                response.lock().unwrap().should_close().then_some(*modal_id)
            })
            .collect::<Vec<_>>();

        for modal_id in modal_ids_to_close {
            self.modals.shift_remove(&modal_id);
        }

        self.responses.clear();

        if let Some(id) = self.modals.keys().last() {
            self.show_modal(ui, *id);
        }
    }

    fn show_modal(&mut self, ui: &mut egui::Ui, modal_id: ModalId) {
        if let Some(guard) = self.modals.get(&modal_id) {
            let mut modal = guard.lock().unwrap();
            let response = egui::Modal::new(egui::Id::new(("app_modal", modal_id)))
                .backdrop_color(color::OVERLAY)
                .frame(style::dialog_frame())
                .show(ui.ctx(), |ui: &mut egui::Ui| {
                    ui.set_min_width(style::DIALOG_MIN_WIDTH);
                    style::dialog_title(ui, modal.title());
                    ui.add_space(12.0);
                    modal.body_ui(ui);
                    ui.add_space(24.0);
                    egui::Sides::new()
                        .show(ui, |_| {}, |ui| modal.actions_ui_boxed(ui))
                        .1
                });

            let should_close = response.should_close();
            let action_response = response.inner;
            let response = action_response.or_else(|| {
                should_close
                    .then(|| modal.cancel_response_boxed())
                    .flatten()
            });

            if let Some(response) = response {
                self.responses.insert(modal_id, Mutex::new(response));
            }
        }
    }
}

trait DynModal: Send + Any {
    fn title(&self) -> String;
    fn body_ui(&mut self, ui: &mut egui::Ui);
    fn actions_ui_boxed(&mut self, ui: &mut egui::Ui) -> Option<Box<dyn ModalResponse>>;
    fn cancel_response_boxed(&self) -> Option<Box<dyn ModalResponse>>;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T> DynModal for T
where
    T: Modal,
{
    fn title(&self) -> String {
        <T as Modal>::title(self)
    }

    fn body_ui(&mut self, ui: &mut egui::Ui) {
        <T as Modal>::body_ui(self, ui)
    }

    fn actions_ui_boxed(&mut self, ui: &mut egui::Ui) -> Option<Box<dyn ModalResponse>> {
        self.actions_ui(ui)
            .map(|r| Box::new(r) as Box<dyn ModalResponse>)
    }

    fn cancel_response_boxed(&self) -> Option<Box<dyn ModalResponse>> {
        T::Response::cancel().map(|r| Box::new(r) as Box<dyn ModalResponse>)
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        <T as Modal>::as_any_mut(self)
    }
}
