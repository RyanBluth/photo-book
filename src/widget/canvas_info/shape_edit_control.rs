use eframe::egui::{self, Ui};
use egui::{ComboBox, Stroke, StrokeKind};

use crate::{theme::color, utils::EditableValueTextEdit};

use super::layers::{
    CanvasShapeKind, Layer,
    LayerContent::{Photo, Shape, TemplatePhoto, TemplateText, Text},
};
use super::property_control::{color_field, field, field_pair, property_group};

pub struct ShapeEditControl<'a> {
    layer: &'a mut Layer,
}

impl<'a> ShapeEditControl<'a> {
    pub fn new(layer: &'a mut Layer) -> Self {
        Self { layer }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let _response: egui::InnerResponse<()> =
            ui.allocate_ui(ui.available_size(), |ui| match &mut self.layer.content {
                Photo(_) | TemplatePhoto { .. } | Text(_) | TemplateText { .. } => {
                    ui.label("No shape layer selected");
                }
                Shape(shape) => {
                    let is_line = matches!(shape.kind, CanvasShapeKind::Line { .. });
                    let (stroke_width, stroke_color) = shape
                        .stroke
                        .map(|(stroke, _)| (stroke.width, stroke.color))
                        .unwrap_or((1.0, color::BLACK));
                    shape
                        .edit_state
                        .update(stroke_width, shape.fill_color, stroke_color);

                    property_group(ui, |ui| {
                        field(ui, "Fill color", |ui| {
                            color_field(
                                ui,
                                &mut shape.fill_color,
                                &mut shape.edit_state.fill_color,
                            );
                        });

                        // Only show stroke kind for non-line shapes
                        if !is_line {
                            field(ui, "Stroke", |ui| {
                                let selected_label = match shape.stroke.map(|(_, kind)| kind) {
                                    Some(StrokeKind::Inside) => "Inside",
                                    Some(StrokeKind::Middle) => "Middle",
                                    Some(StrokeKind::Outside) => "Outside",
                                    None => "None",
                                };

                                let mut current_kind = shape.stroke.map(|(_, kind)| kind);
                                let current_width =
                                    shape.stroke.map(|(stroke, _)| stroke.width).unwrap_or(1.0);
                                let current_stroke_color = shape
                                    .stroke
                                    .map(|(stroke, _)| stroke.color)
                                    .unwrap_or(color::BLACK);

                                ComboBox::from_id_salt("shape_stroke_alignment")
                                    .selected_text(selected_label)
                                    .width(ui.available_width())
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut current_kind, None, "None");
                                        ui.selectable_value(
                                            &mut current_kind,
                                            Some(StrokeKind::Inside),
                                            "Inside",
                                        );
                                        ui.selectable_value(
                                            &mut current_kind,
                                            Some(StrokeKind::Middle),
                                            "Middle",
                                        );
                                        ui.selectable_value(
                                            &mut current_kind,
                                            Some(StrokeKind::Outside),
                                            "Outside",
                                        );
                                    });

                                match current_kind {
                                    Some(kind) => {
                                        shape.stroke = Some((
                                            Stroke::new(current_width, current_stroke_color),
                                            kind,
                                        ))
                                    }
                                    None => shape.stroke = None,
                                }
                            });
                        } else {
                            // For lines, always show stroke controls (no "None" option)
                            if shape.stroke.is_none() {
                                shape.stroke =
                                    Some((Stroke::new(2.0, shape.fill_color), StrokeKind::Middle));
                            }
                        }

                        if let Some((stroke_val, _)) = &mut shape.stroke {
                            let (stroke_width, _) = field_pair(
                                ui,
                                |ui| {
                                    field(ui, "Stroke width", |ui| {
                                        ui.style_mut().spacing.text_edit_width =
                                            ui.available_width();
                                        ui.text_edit_editable_value_singleline(
                                            &mut shape.edit_state.stroke_width,
                                        )
                                    })
                                },
                                |ui| {
                                    field(ui, "Stroke color", |ui| {
                                        color_field(
                                            ui,
                                            &mut stroke_val.color,
                                            &mut shape.edit_state.stroke_color,
                                        );
                                    });
                                },
                            );
                            stroke_val.width = stroke_width;
                        }
                    });
                }
            });
    }
}
