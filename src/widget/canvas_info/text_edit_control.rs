use eframe::{
    egui::{self, RichText, Ui},
    epaint::FontId,
};
use egui::ComboBox;
use strum::IntoEnumIterator;

use crate::utils::EditableValueTextEdit;

use super::layers::{
    Layer,
    LayerContent::{Photo, Shape, TemplatePhoto, TemplateText, Text},
    TextHorizontalAlignment, TextVerticalAlignment,
};
use super::property_control::{color_field, field, field_pair, property_group};

pub struct TextEditControl<'a> {
    layer: &'a mut Layer,
}

impl<'a> TextEditControl<'a> {
    pub fn new(layer: &'a mut Layer) -> Self {
        Self { layer }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let _response: egui::InnerResponse<()> =
            ui.allocate_ui(ui.available_size(), |ui| match &mut self.layer.content {
                Photo(_) | TemplatePhoto { .. } | Shape(_) => {
                    ui.label("No text layer selected");
                }
                Text(text_content)
                | TemplateText {
                    region: _,
                    text: text_content,
                } => {
                    text_content
                        .edit_state
                        .update(text_content.font_size, text_content.color);

                    property_group(ui, |ui| {
                        let (_, new_font_size) = field_pair(
                            ui,
                            |ui| {
                                field(ui, "Font family", |ui| {
                                    ComboBox::from_id_salt("text_font_family")
                                        .selected_text(text_content.font_id.family.to_string())
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
                                                    &mut text_content.font_id,
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
                                        &mut text_content.edit_state.font_size,
                                    )
                                })
                            },
                        );
                        text_content.font_size = new_font_size;

                        field(ui, "Color", |ui| {
                            color_field(
                                ui,
                                &mut text_content.color,
                                &mut text_content.edit_state.color,
                            );
                        });

                        let mut horizontal_alignment = text_content.horizontal_alignment;
                        let mut vertical_alignment = text_content.vertical_alignment;
                        field_pair(
                            ui,
                            |ui| {
                                field(ui, "Horizontal alignment", |ui| {
                                    ComboBox::from_id_salt("text_horizontal_alignment")
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
                                    ComboBox::from_id_salt("text_vertical_alignment")
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
                        text_content.horizontal_alignment = horizontal_alignment;
                        text_content.vertical_alignment = vertical_alignment;
                    });
                }
            });
    }
}
