use std::{fmt::Display, str::FromStr};

use eframe::{
    egui::{self, FontTweak, Grid, RichText, Ui},
    epaint::{FontFamily, FontId, Vec2},
};

use crate::widget::page_canvas::TransformableState;

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

trait EditableValueTextEdit {
    fn text_edit_editable_value_singleline<'a, T>(
        &mut self,
        value: &'a mut EditableValue<T>,
        apply: impl FnOnce(&'a EditableValue<T>) -> (),
    ) -> egui::Response
    where
        T: Display,
        T: FromStr,
        T: Clone;
}

impl EditableValueTextEdit for Ui {
    fn text_edit_editable_value_singleline<'a, T>(
        &mut self,
        value: &'a mut EditableValue<T>,
        apply: impl FnOnce(&'a EditableValue<T>) -> (),
    ) -> egui::Response
    where
        T: Display,
        T: FromStr,
        T: Clone,
    {
        let text_edit_response = self.text_edit_singleline(value.editable_value());

        if text_edit_response.gained_focus() {
            value.begin_editing();
        } else if text_edit_response.lost_focus() {
            value.end_editing();

            apply(value);
        }

        text_edit_response
    }
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

                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(10.0, 5.0);
                    ui.style_mut().spacing.text_edit_width = 80.0;

                    ui.label(RichText::new("Position").heading());

                    ui.horizontal(|ui| {
                        ui.label("x:");

                        ui.text_edit_editable_value_singleline(
                            &mut layer.transform_edit_state.x,
                            |value| {
                                let current_left = layer.photo.transform_state.rect.left_top().x;

                                layer.photo.transform_state.rect = layer
                                    .photo
                                    .transform_state
                                    .rect
                                    .translate(Vec2::new(value.value() - current_left, 0.0));
                            },
                        );

                        ui.label("y:");

                        ui.text_edit_editable_value_singleline(
                            &mut layer.transform_edit_state.y,
                            |value| {
                                let current_top = layer.photo.transform_state.rect.left_top().y;

                                layer.photo.transform_state.rect = layer
                                    .photo
                                    .transform_state
                                    .rect
                                    .translate(Vec2::new(0.0, value.value() - current_top));
                            },
                        );
                    });

                    ui.separator();

                    ui.label(RichText::new("Size").heading());

                    ui.horizontal(|ui| {
                        ui.label("Width:");

                        ui.text_edit_editable_value_singleline(
                            &mut layer.transform_edit_state.width,
                            |value| {
                                layer.photo.transform_state.rect.set_width(value.value());
                            },
                        );

                        ui.label("Height:");

                        ui.text_edit_editable_value_singleline(
                            &mut layer.transform_edit_state.height,
                            |value| {
                                layer.photo.transform_state.rect.set_height(value.value());
                            },
                        );
                    });

                    ui.separator();

                    ui.label(RichText::new("Rotation").heading());

                    ui.horizontal(|ui| {
                        ui.label("Degrees:");

                        ui.text_edit_editable_value_singleline(
                            &mut layer.transform_edit_state.rotation,
                            |value| {
                                layer.photo.transform_state.rotation = value.value().to_radians();
                            },
                        );
                    });
                });
            }
        });
    }
}
