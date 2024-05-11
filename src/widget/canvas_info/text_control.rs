use eframe::{
    egui::{self, RichText, Ui},
    epaint::{FontId, Vec2},
};
use egui::ComboBox;
use strum::IntoEnumIterator;

use crate::{template::Template, utils::EditableValueTextEdit, widget::page_canvas::CanvasPhoto};

use super::layers::{
    CanvasTextHorizontalAlignment, CanvasTextVerticalAlignment, Layer, LayerContent::{Photo, TemplatePhoto, TemplateText, Text}
};

pub struct TextControlState<'a> {
    layer: &'a mut Layer,
}

impl<'a> TextControlState<'a> {
    pub fn new(layer: &'a mut Layer) -> Self {
        Self { layer }
    }
}

pub struct TextControl<'a> {
    state: TextControlState<'a>,
}

impl<'a> TextControl<'a> {
    pub fn new(state: TextControlState<'a>) -> Self {
        Self { state }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let _response: egui::InnerResponse<()> =
            ui.allocate_ui(ui.available_size(), |ui| match self.state.layer.content {
                Photo(_) | TemplatePhoto { .. } => {
                    ui.label("No text layer selected");
                }
                Text(ref mut text_content)
                | TemplateText {
                    region: _,
                    text: ref mut text_content,
                } => {
                    text_content.edit_state.update(text_content.font_size);

                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(10.0, 5.0);
                        ui.style_mut().spacing.text_edit_width = 80.0;

                        ui.label(RichText::new("Text").heading());

                        ui.horizontal(|ui| {
                            let text = &mut self.state.layer.content;
                            match text {
                                Text(text) | TemplateText { region: _, text } => {
                                    let mut new_text = text.text.clone();
                                    ui.label("Text:");
                                    ui.text_edit_singleline(&mut new_text);
                                    text.text = new_text;
                                }
                                _ => (),
                            }
                        });

                        ui.horizontal(|ui| {
                            let text = &mut self.state.layer.content;
                            match text {
                                Text(text) | TemplateText { region: _, text } => {
                                    ui.label("Font Size:");

                                    let new_font_size = ui.text_edit_editable_value_singleline(
                                        &mut text.edit_state.font_size,
                                    );
                                    text.font_size = new_font_size;
                                }
                                _ => (),
                            }
                        });

                        ui.horizontal(|ui| {
                            let text = &mut self.state.layer.content;
                            match text {
                                Text(text) | TemplateText { region: _, text } => {
                                    ui.label("Font Family:");

                                    ComboBox::from_label("Font Family")
                                        .selected_text(format!("{}", text.font_id.family))
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
                                                    &mut text.font_id,
                                                    font_id.clone(),
                                                    RichText::new(font_id.family.to_string())
                                                        .font(font_id.clone()),
                                                );
                                            }
                                        });
                                }
                                _ => (),
                            }
                        });

                        ui.horizontal(|ui| {
                            let text = &mut self.state.layer.content;
                            match text {
                                Text(text) | TemplateText { region: _, text } => {
                                    ui.label("Color:");

                                    ui.color_edit_button_srgba(&mut text.color);
                                }
                                _ => (),
                            }
                        });

                        ui.horizontal(|ui| {
                            let text = &mut self.state.layer.content;
                            match text {
                                Text(text) | TemplateText { region: _, text } => {

                                    let mut current_alignment = match text.layout.cross_align {
                                        egui::Align::Min => CanvasTextHorizontalAlignment::Left,
                                        egui::Align::Center => CanvasTextHorizontalAlignment::Center,
                                        egui::Align::Max => CanvasTextHorizontalAlignment::Right,
                                    };

                                    ComboBox::from_label("Horizontal Alignment")
                                        .selected_text(format!("{}", current_alignment))
                                        .show_ui(ui, |ui| {
                                            for alignment in CanvasTextHorizontalAlignment::iter() {
                                                ui.selectable_value(
                                                    &mut current_alignment,
                                                    alignment.clone(),
                                                    RichText::new(alignment.to_string()),
                                                );
                                            }
                                        });

                                    text.layout.cross_align = match current_alignment {
                                        CanvasTextHorizontalAlignment::Left => egui::Align::Min,
                                        CanvasTextHorizontalAlignment::Center => egui::Align::Center,
                                        CanvasTextHorizontalAlignment::Right => egui::Align::Max,
                                    };
                                }
                                _ => (),
                            }
                        });

                        ui.horizontal(|ui| {
                            let text = &mut self.state.layer.content;
                            match text {
                                Text(text) | TemplateText { region: _, text } => {
                                    let mut current_alignment = match text.layout.main_align {
                                        egui::Align::Min => CanvasTextVerticalAlignment::Top,
                                        egui::Align::Center => CanvasTextVerticalAlignment::Center,
                                        egui::Align::Max => CanvasTextVerticalAlignment::Bottom,
                                    };

                                    ComboBox::from_label("Vertical Alignment")
                                        .selected_text(format!("{}", current_alignment))
                                        .show_ui(ui, |ui| {
                                            for alignment in CanvasTextVerticalAlignment::iter() {
                                                ui.selectable_value(
                                                    &mut current_alignment,
                                                    alignment.clone(),
                                                    RichText::new(alignment.to_string()),
                                                );
                                            }
                                        });

                                    text.layout.main_align = match current_alignment {
                                        CanvasTextVerticalAlignment::Top => egui::Align::Min,
                                        CanvasTextVerticalAlignment::Center => egui::Align::Center,
                                        CanvasTextVerticalAlignment::Bottom => egui::Align::Max,
                                    };
                                }
                                _ => (),
                            }
                        });
                    });
                }
            });
    }
}
