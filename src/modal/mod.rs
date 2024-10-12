use std::{any::Any, sync::{Arc, Mutex}};

use egui::{Align::Min, Button, Color32, Layout, ProgressBar, Vec2};
use indexmap::IndexMap;

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    id::{next_modal_id, ModalId},
    model::{edit_state::EditablePage, page::Page},
    project_settings::ProjectSettingsManager,
    widget::canvas_info::page_info::{PageInfo, PageInfoState},
};

pub mod manager;
pub mod basic;
pub mod page_settings;
pub mod progress;

pub struct ModalAction {
    pub func: Mutex<Box<dyn Fn() -> () + Send>>,
    pub label: String,
    pub is_enabled: bool,
}

pub enum ModalResponse {
    Dismiss,
    None,
}

pub trait Modal: Send + Any {
    fn title(&self) -> String;

    fn ui(&mut self, ui: &mut egui::Ui) -> ModalResponse;

    fn dismiss_label(&self) -> String;

    fn actions(&self) -> Option<Vec<ModalAction>>;

    fn as_any_mut(&mut self) -> &mut dyn Any;
}