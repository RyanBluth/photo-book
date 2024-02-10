use std::{fmt::Display, str::FromStr};

use eframe::{
    egui::{self, FontTweak, Grid, RichText, Ui},
    epaint::{FontFamily, FontId, Vec2},
};

use crate::{utils::EditableValueTextEdit, widget::page_canvas::TransformableState};

use super::layers::{EditableValue, Layer};

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
                layer.transform_edit_state.update(&layer.transform_state);

                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(10.0, 5.0);
                    ui.style_mut().spacing.text_edit_width = 80.0;

                    ui.label(RichText::new("Position").heading());

                    ui.horizontal(|ui| {
                        ui.label("x:");

                        let new_x = ui
                            .text_edit_editable_value_singleline(&mut layer.transform_edit_state.x);

                        let current_left = layer.transform_state.rect.left_top().x;

                        layer.transform_state.rect = layer
                            .transform_state
                            .rect
                            .translate(Vec2::new(new_x - current_left, 0.0));

                        ui.label("y:");

                        let new_y = ui
                            .text_edit_editable_value_singleline(&mut layer.transform_edit_state.y);

                        let current_top = layer.transform_state.rect.left_top().y;

                        layer.transform_state.rect = layer
                            .transform_state
                            .rect
                            .translate(Vec2::new(0.0, new_y - current_top));
                    });

                    ui.separator();

                    ui.label(RichText::new("Size").heading());

                    ui.horizontal(|ui| {
                        ui.label("Width:");

                        let new_width = ui.text_edit_editable_value_singleline(
                            &mut layer.transform_edit_state.width,
                        );

                        layer.transform_state.rect.set_width(new_width);

                        ui.label("Height:");

                        let new_height = ui.text_edit_editable_value_singleline(
                            &mut layer.transform_edit_state.height,
                        );

                        layer.transform_state.rect.set_height(new_height);
                    });

                    ui.separator();

                    ui.label(RichText::new("Rotation").heading());

                    ui.horizontal(|ui| {
                        ui.label("Degrees:");

                        let new_rotation = ui.text_edit_editable_value_singleline(
                            &mut layer.transform_edit_state.rotation,
                        );

                        layer.transform_state.rotation = new_rotation.to_radians();
                    });
                });
            }
        });
    }
}
