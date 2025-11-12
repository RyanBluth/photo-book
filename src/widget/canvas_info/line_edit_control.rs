use eframe::egui::{self, RichText, Ui};

use crate::utils::EditableValueTextEdit;

use super::layers::{
    Layer,
    LayerContent::{Photo, Shape, TemplatePhoto, TemplateText, Text},
};

pub struct LineEditControl<'a> {
    layer: &'a mut Layer,
}

impl<'a> LineEditControl<'a> {
    pub fn new(layer: &'a mut Layer) -> Self {
        Self { layer }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let _response: egui::InnerResponse<()> = ui.allocate_ui(ui.available_size(), |ui| {
            match &mut self.layer.content {
                Photo(_) | TemplatePhoto { .. } | Text(_) | TemplateText { .. } => {
                    ui.label("No line layer selected");
                }
                Shape(shape) => {
                    if let Some((stroke, _)) = &mut shape.stroke {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing = egui::Vec2::new(10.0, 5.0);

                            ui.label(RichText::new("Line").heading());

                            ui.horizontal(|ui| {
                                ui.label("Color:");
                                ui.color_edit_button_srgba(&mut stroke.color);
                            });

                            ui.horizontal(|ui| {
                                ui.label("Thickness:");
                                ui.text_edit_editable_value_singleline(
                                    &mut shape.edit_state.stroke_width,
                                );
                                stroke.width = shape.edit_state.stroke_width.value();
                            });
                        });
                    }
                }
            }
        });
    }
}
