
use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    model::{edit_state::EditablePage, page::Page},
    project_settings::ProjectSettingsManager,
    widget::canvas_info::page_info::{PageInfo, PageInfoState},
};

use super::{Modal, ModalActionResponse};

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

    fn body_ui(&mut self, ui: &mut egui::Ui) {
        PageInfo::new(&mut PageInfoState::new(&mut self.editable_page)).show(ui);
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn actions_ui(&mut self, ui: &mut egui::Ui) -> ModalActionResponse {
        if ui.button("Cancel").clicked() {
            return ModalActionResponse::Cancel;
        }

        if ui.button("Save").clicked() {
            let project_settings_manager: Singleton<ProjectSettingsManager> = Dependency::get();
            project_settings_manager.with_lock_mut(|project_settings_manager| {
                project_settings_manager.project_settings.default_page =
                    Some(self.editable_page.value.clone());
            });
            return ModalActionResponse::Confirm;
        }

        return ModalActionResponse::None;
    }
}
