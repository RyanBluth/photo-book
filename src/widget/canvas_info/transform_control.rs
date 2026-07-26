use eframe::{
    egui::{RichText, Ui},
    epaint::Vec2,
};
use std::{fmt::Display, str::FromStr};

use crate::{
    model::editable_value::EditableValue,
    theme::color,
    utils::EditableValueTextEdit,
    widget::{edit_response::EditResponse, field_pair::FieldPair},
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

    pub fn show(&mut self, ui: &mut Ui) -> EditResponse {
        let is_template = self.state.layer.content.is_template();
        let mut response = EditResponse::none();

        ui.add_enabled_ui(!is_template, |ui| {
            self.state
                .layer
                .transform_edit_state
                .update(&self.state.layer.transform_state);

            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 12.0);

                response |= self.show_position(ui);
                response |= self.show_size(ui);
                response |= self.show_rotation(ui);
            });
        });
        response
    }

    fn show_position(&mut self, ui: &mut Ui) -> EditResponse {
        let ((new_x, x_response), (new_y, y_response)) = FieldPair::new().show(
            ui,
            |ui| Self::field(ui, "X", &mut self.state.layer.transform_edit_state.x),
            |ui| Self::field(ui, "Y", &mut self.state.layer.transform_edit_state.y),
        );

        if let Some(new_x) = new_x {
            let current_left = self.state.layer.transform_state.rect.left_top().x;
            self.state.layer.transform_state.rect = self
                .state
                .layer
                .transform_state
                .rect
                .translate(Vec2::new(new_x - current_left, 0.0));
        }

        if let Some(new_y) = new_y {
            let current_top = self.state.layer.transform_state.rect.left_top().y;
            self.state.layer.transform_state.rect = self
                .state
                .layer
                .transform_state
                .rect
                .translate(Vec2::new(0.0, new_y - current_top));
        }
        x_response | y_response
    }

    fn show_size(&mut self, ui: &mut Ui) -> EditResponse {
        let ((new_width, width_response), (new_height, height_response)) = FieldPair::new().show(
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

        if let Some(new_width) = new_width {
            self.state.layer.transform_state.rect.set_width(new_width);
        }
        if let Some(new_height) = new_height {
            self.state.layer.transform_state.rect.set_height(new_height);
        }
        width_response | height_response
    }

    fn show_rotation(&mut self, ui: &mut Ui) -> EditResponse {
        let (new_rotation, response) = Self::field(
            ui,
            "Rotation",
            &mut self.state.layer.transform_edit_state.rotation,
        );
        if let Some(new_rotation) = new_rotation {
            self.state.layer.transform_state.rotation = new_rotation.to_radians();
        }
        response
    }

    fn field<T>(ui: &mut Ui, label: &str, value: &mut EditableValue<T>) -> (Option<T>, EditResponse)
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
            let response = ui.text_edit_editable_value_singleline_live(value);
            let edit_response = EditResponse::text(&response.response, response.value.is_some());
            (response.value, edit_response)
        })
        .inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, Rect};
    use egui_kittest::{Harness, kittest::Queryable};

    #[test]
    fn passive_sync_does_not_rewrite_transform_geometry() {
        let mut layer = Layer::new_rectangle_shape_layer();
        layer.transform_state.rect = Rect::from_min_max(
            Pos2::new(531.28186, 145.04425),
            Pos2::new(3007.9978, 3710.9019),
        );
        layer.transform_state.rotation = -0.37608597;
        let expected = layer.transform_state.clone();

        let mut harness = Harness::new_ui_state(
            |ui, layer| {
                let _ = TransformControl::new(TransformControlState::new(layer)).show(ui);
            },
            layer,
        );
        harness.run();

        assert_eq!(harness.state().transform_state, expected);
    }

    #[test]
    fn valid_field_edit_updates_transform_geometry_while_focused() {
        let layer = Layer::new_rectangle_shape_layer();
        let mut harness = Harness::new_ui_state(
            |ui, layer| {
                let _ = TransformControl::new(TransformControlState::new(layer)).show(ui);
            },
            layer,
        );

        harness
            .get_all_by_role(egui::accesskit::Role::TextInput)
            .next()
            .unwrap()
            .click();
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.run();
        harness
            .get_all_by_role(egui::accesskit::Role::TextInput)
            .next()
            .unwrap()
            .type_text("812.5");
        harness.run();

        assert_eq!(
            harness.state().transform_state.rect.left(),
            812.5,
            "valid input should update the document before another control can start"
        );
    }
}
