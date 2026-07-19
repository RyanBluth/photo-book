use eframe::egui::{self, Ui};

use crate::utils::EditableValueTextEdit;

use super::layers::{
    Layer,
    LayerContent::{Photo, Shape, TemplatePhoto, TemplateText, Text},
};
use super::property_control::{color_field, field, field_pair, property_group};

pub struct LineEditControl<'a> {
    layer: &'a mut Layer,
}

impl<'a> LineEditControl<'a> {
    pub fn new(layer: &'a mut Layer) -> Self {
        Self { layer }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let _response: egui::InnerResponse<()> =
            ui.allocate_ui(ui.available_size(), |ui| match &mut self.layer.content {
                Photo(_) | TemplatePhoto { .. } | Text(_) | TemplateText { .. } => {
                    ui.label("No line layer selected");
                }
                Shape(shape) => {
                    if let Some((stroke, _)) = &mut shape.stroke {
                        shape
                            .edit_state
                            .update(stroke.width, shape.fill_color, stroke.color);
                        property_group(ui, |ui| {
                            let (_, width) = field_pair(
                                ui,
                                |ui| {
                                    field(ui, "Color", |ui| {
                                        color_field(
                                            ui,
                                            &mut stroke.color,
                                            &mut shape.edit_state.stroke_color,
                                        );
                                    });
                                },
                                |ui| {
                                    field(ui, "Thickness", |ui| {
                                        ui.style_mut().spacing.text_edit_width =
                                            ui.available_width();
                                        ui.text_edit_editable_value_singleline(
                                            &mut shape.edit_state.stroke_width,
                                        )
                                    })
                                },
                            );
                            stroke.width = width;
                        });
                    }
                }
            });
    }
}
