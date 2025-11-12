use eframe::{
    egui::{RichText, Ui},
    epaint::{FontId, Vec2},
};
use egui::ComboBox;
use strum::IntoEnumIterator;

use crate::utils::EditableValueTextEdit;

use super::layers::{TextHorizontalAlignment, TextToolSettings, TextVerticalAlignment};

pub struct TextToolControl<'a> {
    settings: &'a mut TextToolSettings,
}

impl<'a> TextToolControl<'a> {
    pub fn new(settings: &'a mut TextToolSettings) -> Self {
        Self { settings }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        self.settings.edit_state.update(self.settings.font_size);

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 5.0);
            ui.style_mut().spacing.text_edit_width = 80.0;

            ui.label(RichText::new("Text Tool Settings").heading());

            ui.horizontal(|ui| {
                ui.label("Font Size:");
                let new_font_size =
                    ui.text_edit_editable_value_singleline(&mut self.settings.edit_state.font_size);
                self.settings.font_size = new_font_size;
            });

            ui.horizontal(|ui| {
                ui.label("Font Family:");

                ComboBox::from_label("Font Family")
                    .selected_text(format!("{}", self.settings.font_id.family))
                    .show_ui(ui, |ui| {
                        let fonts = ui.ctx().fonts(|fonts| {
                            fonts
                                .families()
                                .iter()
                                .map(|family| FontId::new(20.0, family.clone()))
                                .collect::<Vec<FontId>>()
                        });

                        for font_id in &fonts {
                            ui.selectable_value(
                                &mut self.settings.font_id,
                                font_id.clone(),
                                RichText::new(font_id.family.to_string()).font(font_id.clone()),
                            );
                        }
                    });
            });

            ui.horizontal(|ui| {
                ui.label("Color:");
                ui.color_edit_button_srgba(&mut self.settings.color);
            });

            ui.horizontal(|ui| {
                let mut current_alignment = self.settings.horizontal_alignment;

                ComboBox::from_label("Horizontal Alignment")
                    .selected_text(format!("{}", current_alignment))
                    .show_ui(ui, |ui| {
                        for alignment in TextHorizontalAlignment::iter() {
                            ui.selectable_value(
                                &mut current_alignment,
                                alignment,
                                RichText::new(alignment.to_string()),
                            );
                        }
                    });

                self.settings.horizontal_alignment = current_alignment;
            });

            ui.horizontal(|ui| {
                let mut current_alignment = self.settings.vertical_alignment;

                ComboBox::from_label("Vertical Alignment")
                    .selected_text(format!("{}", current_alignment))
                    .show_ui(ui, |ui| {
                        for alignment in TextVerticalAlignment::iter() {
                            ui.selectable_value(
                                &mut current_alignment,
                                alignment,
                                RichText::new(alignment.to_string()),
                            );
                        }
                    });

                self.settings.vertical_alignment = current_alignment;
            });
        });
    }
}
