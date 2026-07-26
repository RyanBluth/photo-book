use eframe::{
    egui::{RichText, Ui},
    epaint::FontId,
};
use egui::ComboBox;
use strum::IntoEnumIterator;

use crate::utils::EditableValueTextEdit;

use super::layers::{TextHorizontalAlignment, TextToolSettings, TextVerticalAlignment};
use super::property_control::{color_field, field, field_pair, property_group};

pub struct TextToolControl<'a> {
    settings: &'a mut TextToolSettings,
}

impl<'a> TextToolControl<'a> {
    pub fn new(settings: &'a mut TextToolSettings) -> Self {
        Self { settings }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        self.settings
            .edit_state
            .update(self.settings.font_size, self.settings.color);

        property_group(ui, |ui| {
            let (_, new_font_size) = field_pair(
                ui,
                |ui| {
                    field(ui, "Font family", |ui| {
                        ComboBox::from_id_salt("text_tool_font_family")
                            .selected_text(self.settings.font_id.family.to_string())
                            .width(ui.available_width())
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
                                        RichText::new(font_id.family.to_string())
                                            .font(font_id.clone()),
                                    );
                                }
                            });
                    })
                },
                |ui| {
                    field(ui, "Font size", |ui| {
                        ui.style_mut().spacing.text_edit_width = ui.available_width();
                        ui.text_edit_editable_value_singleline(
                            &mut self.settings.edit_state.font_size,
                        )
                    })
                },
            );
            self.settings.font_size = new_font_size;

            field(ui, "Color", |ui| {
                let _ = color_field(
                    ui,
                    &mut self.settings.color,
                    &mut self.settings.edit_state.color,
                );
            });

            let mut horizontal_alignment = self.settings.horizontal_alignment;
            let mut vertical_alignment = self.settings.vertical_alignment;
            field_pair(
                ui,
                |ui| {
                    field(ui, "Horizontal alignment", |ui| {
                        ComboBox::from_id_salt("text_tool_horizontal_alignment")
                            .selected_text(horizontal_alignment.to_string())
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                for alignment in TextHorizontalAlignment::iter() {
                                    ui.selectable_value(
                                        &mut horizontal_alignment,
                                        alignment,
                                        alignment.to_string(),
                                    );
                                }
                            });
                    });
                },
                |ui| {
                    field(ui, "Vertical alignment", |ui| {
                        ComboBox::from_id_salt("text_tool_vertical_alignment")
                            .selected_text(vertical_alignment.to_string())
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                for alignment in TextVerticalAlignment::iter() {
                                    ui.selectable_value(
                                        &mut vertical_alignment,
                                        alignment,
                                        alignment.to_string(),
                                    );
                                }
                            });
                    });
                },
            );
            self.settings.horizontal_alignment = horizontal_alignment;
            self.settings.vertical_alignment = vertical_alignment;
        });
    }
}
