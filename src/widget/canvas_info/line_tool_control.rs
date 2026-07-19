use eframe::egui::Ui;

use crate::utils::EditableValueTextEdit;

use super::layers::LineToolSettings;
use super::property_control::{color_field, field, field_pair, property_group};

pub struct LineToolControl<'a> {
    settings: &'a mut LineToolSettings,
}

impl<'a> LineToolControl<'a> {
    pub fn new(settings: &'a mut LineToolSettings) -> Self {
        Self { settings }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        self.settings.edit_state.update(
            self.settings.width,
            self.settings.color,
            self.settings.color,
        );
        property_group(ui, |ui| {
            let (_, width) = field_pair(
                ui,
                |ui| {
                    field(ui, "Color", |ui| {
                        color_field(
                            ui,
                            &mut self.settings.color,
                            &mut self.settings.edit_state.stroke_color,
                        );
                    });
                },
                |ui| {
                    field(ui, "Thickness", |ui| {
                        ui.style_mut().spacing.text_edit_width = ui.available_width();
                        ui.text_edit_editable_value_singleline(
                            &mut self.settings.edit_state.stroke_width,
                        )
                    })
                },
            );
            self.settings.width = width;
        });
    }
}
