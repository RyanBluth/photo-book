use eframe::egui::{RichText, Ui};

use crate::utils::EditableValueTextEdit;

use super::layers::LineToolSettings;

pub struct LineToolControl<'a> {
    settings: &'a mut LineToolSettings,
}

impl<'a> LineToolControl<'a> {
    pub fn new(settings: &'a mut LineToolSettings) -> Self {
        Self { settings }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::new(10.0, 5.0);

            ui.label(RichText::new("Line Tool Settings").heading());

            ui.horizontal(|ui| {
                ui.label("Color:");
                ui.color_edit_button_srgba(&mut self.settings.color);
            });

            ui.horizontal(|ui| {
                ui.label("Thickness:");
                ui.text_edit_editable_value_singleline(&mut self.settings.edit_state.stroke_width);
                self.settings.width = self.settings.edit_state.stroke_width.value();
            });
        });
    }
}
