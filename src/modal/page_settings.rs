use crate::{
    dep, dep_mut,
    model::edit_state::EditablePage,
    project_settings::ProjectSettingsManager,
    theme::style,
    widget::canvas_info::page_info::{PageInfo, PageInfoState},
};

use super::{Modal, ModalActionResponse};

#[derive(Debug, Clone)]
pub struct PageSettingsModal {
    editable_page: EditablePage,
}

impl PageSettingsModal {
    pub fn new() -> Self {
        let current_page = dep!(ProjectSettingsManager, |settings| {
            settings
                .project_settings
                .default_page
                .clone()
                .unwrap_or_default()
        });
        Self {
            editable_page: EditablePage::new(current_page),
        }
    }
}

impl Modal for PageSettingsModal {
    type Response = ModalActionResponse;

    fn title(&self) -> String {
        "Page Settings".to_string()
    }

    fn body_ui(&mut self, ui: &mut egui::Ui) {
        PageInfo::new(&mut PageInfoState::new(&mut self.editable_page)).show(ui);
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn actions_ui(&mut self, ui: &mut egui::Ui) -> Option<Self::Response> {
        if style::primary_button(ui, "Save").clicked() {
            dep_mut!(ProjectSettingsManager, |project_settings_manager| {
                project_settings_manager.project_settings.default_page =
                    Some(self.editable_page.value.clone());
            });
            return Some(ModalActionResponse::Confirm);
        }

        if ui.button("Cancel").clicked() {
            return Some(ModalActionResponse::Cancel);
        }

        None
    }
}
