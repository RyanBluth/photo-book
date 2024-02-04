use eframe::egui::{self, Grid, Ui};

use crate::widget::page_canvas::TransformableState;

use super::layers::Layer;

pub struct TransformControlState<'a> {
    layer: &'a mut Option<&'a mut Layer>,
}

impl<'a> TransformControlState<'a> {
    pub fn new(layer: &'a mut Option<&'a mut Layer>) -> Self {
        Self { layer }
    }
}

pub struct TransformControl<'a> {
    state: TransformControlState<'a>,
}

impl<'a> TransformControl<'a> {
    pub fn new(state: TransformControlState<'a>) -> Self {
        Self { state }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let response = ui.allocate_ui(ui.available_size(), |ui| match self.state.layer {
            None => {
                ui.label("No layer selected");
                return;
            }
            Some(layer) => {
                layer
                    .transform_edit_state
                    .update(&layer.photo.transform_state);

                Grid::new("transform_controls_grid").show(ui, |ui| {
                    ui.label("Position");
                    ui.end_row();

                    ui.horizontal(|ui| {
                        ui.set_width(100.0);
                        ui.label("X");
                        if ui
                            .text_edit_singleline(layer.transform_edit_state.x.editable_value())
                            .changed()
                        {
                            layer.transform_edit_state.x.apply_edit();
                            layer.photo.transform_state.rect.set_left(
                                layer.transform_edit_state.x.value(),
                            );
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.set_width(100.0);
                        ui.label("Y");
                        ui.text_edit_singleline(
                            &mut layer.photo.transform_state.rect.left_top().y.to_string(),
                        );
                    });

                    ui.end_row();

                    ui.label("Size");

                    ui.end_row();

                    ui.horizontal(|ui| {
                        ui.set_width(100.0);
                        ui.label("Width");
                        ui.text_edit_singleline(
                            &mut layer.photo.transform_state.rect.size().x.to_string(),
                        )
                    });

                    ui.horizontal(|ui| {
                        ui.set_width(100.0);
                        ui.label("Height");
                        ui.text_edit_singleline(
                            &mut layer.photo.transform_state.rect.size().y.to_string(),
                        );
                    });

                    ui.end_row();

                    ui.label("Rotation");

                    ui.end_row();

                    ui.horizontal(|ui| {
                        ui.set_width(100.0);
                        ui.label("Degrees");
                        ui.text_edit_singleline(
                            &mut layer
                                .photo
                                .transform_state
                                .rotation
                                .to_degrees()
                                .to_string(),
                        );
                    });
                });
            }
        });
    }
}
