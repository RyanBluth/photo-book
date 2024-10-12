use std::sync::Mutex;

use crate::{dependencies::{Dependency, Singleton, SingletonFor}, model::{edit_state::EditablePage, page::Page}, project_settings::ProjectSettingsManager, widget::canvas_info::page_info::{PageInfo, PageInfoState}};

use super::{ModalAction, ModalResponse, Modal};


pub struct PageSettingsModal {
    editable_page: EditablePage,
}

impl PageSettingsModal {
    pub fn new() -> Self {
        Self {
            editable_page: EditablePage::new(Page::default()),
        }
    }
}

impl Modal for PageSettingsModal {
    fn title(&self) -> String {
        "Page Settings".to_string()
    }

    fn ui(&mut self, ui: &mut egui::Ui) -> ModalResponse {
        PageInfo::new(&mut PageInfoState::new(&mut self.editable_page)).show(ui);

        ModalResponse::None
    }

    fn dismiss_label(&self) -> String {
        "Cancel".to_string()
    }

    fn actions(&self) -> Option<Vec<ModalAction>> {
        Some(vec![ModalAction {
            func: Mutex::new(Box::new(|| {
                let project_settings_manager: Singleton<ProjectSettingsManager> = Dependency::get();
                project_settings_manager.with_lock_mut(|project_settings_manager| {
                    // Set page
                });
            })),
            label: "Save".to_string(),
            is_enabled: true,
        }])
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
