use eframe::{
    egui::{RichText, Ui},
    epaint::Vec2,
};
use std::{fmt::Display, str::FromStr};

use crate::{
    model::editable_value::EditableValue, theme::color, utils::EditableValueTextEdit,
    widget::field_pair::FieldPair,
};

use super::layers::Layer;

pub struct TransformControlState<'a> {
    layer: &'a mut Layer,
}

impl<'a> TransformControlState<'a> {
    pub fn new(layer: &'a mut Layer) -> Self {
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
        let is_template = self.state.layer.content.is_template();

        ui.add_enabled_ui(!is_template, |ui| {
            self.state
                .layer
                .transform_edit_state
                .update(&self.state.layer.transform_state);

            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 12.0);

                self.show_position(ui);
                self.show_size(ui);
                self.show_rotation(ui);
            });
        });
    }

    fn show_position(&mut self, ui: &mut Ui) {
        let (new_x, new_y) = FieldPair::new().show(
            ui,
            |ui| Self::field(ui, "X", &mut self.state.layer.transform_edit_state.x),
            |ui| Self::field(ui, "Y", &mut self.state.layer.transform_edit_state.y),
        );

        let current_left = self.state.layer.transform_state.rect.left_top().x;
        self.state.layer.transform_state.rect = self
            .state
            .layer
            .transform_state
            .rect
            .translate(Vec2::new(new_x - current_left, 0.0));

        let current_top = self.state.layer.transform_state.rect.left_top().y;
        self.state.layer.transform_state.rect = self
            .state
            .layer
            .transform_state
            .rect
            .translate(Vec2::new(0.0, new_y - current_top));
    }

    fn show_size(&mut self, ui: &mut Ui) {
        let (new_width, new_height) = FieldPair::new().show(
            ui,
            |ui| {
                Self::field(
                    ui,
                    "Width",
                    &mut self.state.layer.transform_edit_state.width,
                )
            },
            |ui| {
                Self::field(
                    ui,
                    "Height",
                    &mut self.state.layer.transform_edit_state.height,
                )
            },
        );

        self.state.layer.transform_state.rect.set_width(new_width);
        self.state.layer.transform_state.rect.set_height(new_height);
    }

    fn show_rotation(&mut self, ui: &mut Ui) {
        let new_rotation = Self::field(
            ui,
            "Rotation",
            &mut self.state.layer.transform_edit_state.rotation,
        );

        self.state.layer.transform_state.rotation = new_rotation.to_radians();
    }

    fn field<T>(ui: &mut Ui, label: &str, value: &mut EditableValue<T>) -> T
    where
        T: Display + FromStr + Clone,
    {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.label(
                RichText::new(label)
                    .small()
                    .strong()
                    .color(color::CONTROL_TEXT),
            );
            ui.style_mut().spacing.text_edit_width = ui.available_width();
            ui.text_edit_editable_value_singleline(value)
        })
        .inner
    }
}
