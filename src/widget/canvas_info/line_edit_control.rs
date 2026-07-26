use eframe::egui::Ui;

use crate::{utils::EditableValueTextEdit, widget::edit_response::EditResponse};

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

    pub fn show(&mut self, ui: &mut Ui) -> EditResponse {
        ui.allocate_ui(ui.available_size(), |ui| match &mut self.layer.content {
            Photo(_) | TemplatePhoto { .. } | Text(_) | TemplateText { .. } => {
                ui.label("No line layer selected");
                EditResponse::none()
            }
            Shape(shape) => {
                let mut edit_response = EditResponse::none();
                if let Some((stroke, _)) = &mut shape.stroke {
                    shape
                        .edit_state
                        .update(stroke.width, shape.fill_color, stroke.color);
                    property_group(ui, |ui| {
                        let (_, width_response) = field_pair(
                            ui,
                            |ui| {
                                field(ui, "Color", |ui| {
                                    edit_response |= color_field(
                                        ui,
                                        &mut stroke.color,
                                        &mut shape.edit_state.stroke_color,
                                    );
                                });
                            },
                            |ui| {
                                field(ui, "Thickness", |ui| {
                                    ui.style_mut().spacing.text_edit_width = ui.available_width();
                                    ui.text_edit_editable_value_singleline_live(
                                        &mut shape.edit_state.stroke_width,
                                    )
                                })
                            },
                        );
                        if let Some(width) = width_response.value {
                            stroke.width = width;
                        }
                        edit_response |= EditResponse::text(
                            &width_response.response,
                            width_response.value.is_some(),
                        );
                    });
                }
                edit_response
            }
        })
        .inner
    }
}
