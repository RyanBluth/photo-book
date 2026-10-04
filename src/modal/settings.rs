use egui::{Align, Button, Layout, ScrollArea, TextEdit, Ui, Widget, vec2};

use super::{Modal, ModalActionResponse};

#[derive(Debug, Clone, Default)]
pub struct SettingsModal {
    search: String,
    example_enabled: bool,
}

impl SettingsModal {
    pub fn new() -> Self {
        Self::default()
    }

    fn sidebar_ui(&mut self, ui: &mut Ui) {
        ui.add(
            TextEdit::singleline(&mut self.search)
                .hint_text("Search settings...")
                .desired_width(f32::INFINITY),
        );
        ui.add_space(12.0);
        ui.add_sized(
            [ui.available_width(), 32.0],
            Button::selectable(true, "General"),
        );
    }

    fn content_ui(&mut self, ui: &mut Ui) {
        ui.weak("User");
        ui.add_space(16.0);
        ui.heading("General");
        ui.add_space(12.0);
        ui.weak("General Settings");
        ui.separator();
        ui.add_space(12.0);

        let query = self.search.trim().to_lowercase();
        if !"general example setting placeholder".contains(&query) {
            ui.weak("No settings match your search.");
            return;
        }

        egui::Sides::new().show(
            ui,
            |ui| {
                ui.strong("Example setting");
                ui.add_space(4.0);
                ui.weak("A placeholder setting with no effect on the application.");
            },
            |ui| {
                ui.checkbox(&mut self.example_enabled, "Enabled");
            },
        );
        ui.add_space(16.0);
        ui.separator();
    }
}

impl Widget for &mut SettingsModal {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let screen_size = ui.ctx().content_rect().size();
        let width = (screen_size.x - 80.0).clamp(400.0, 900.0);
        let height = (screen_size.y * 0.8 - 120.0).max(160.0);
        ui.set_width(width);

        ui.horizontal_top(|ui| {
            // Bound the separator's fill height independently of the modal's last size.
            ui.set_height(height);
            ui.allocate_ui_with_layout(vec2(180.0, height), Layout::top_down(Align::Min), |ui| {
                ui.set_width(180.0);
                ui.set_min_height(height);
                self.sidebar_ui(ui);
            });
            ui.separator();
            ui.add_space(12.0);
            ui.allocate_ui_with_layout(
                vec2(ui.available_width(), height),
                Layout::top_down(Align::Min),
                |ui| {
                    ScrollArea::vertical()
                        .id_salt("settings_content")
                        .max_height(height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.content_ui(ui));
                },
            );
        })
        .response
    }
}

impl Modal for SettingsModal {
    type Response = ModalActionResponse;

    fn title(&self) -> String {
        "Settings".to_string()
    }

    fn body_ui(&mut self, ui: &mut Ui) {
        ui.add(self);
    }

    fn actions_ui(&mut self, ui: &mut Ui) -> Option<Self::Response> {
        ui.button("Close")
            .clicked()
            .then_some(ModalActionResponse::_Close)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
