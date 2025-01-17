use eframe::{
    egui::{RichText, Ui},
    epaint::Vec2,
};

use crate::utils::EditableValueTextEdit;

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
        let _response = ui.allocate_ui(ui.available_size(), |ui| {
            if self.state.layer.content.is_template() {
                ui.set_enabled(false);
            }

            self.state
                .layer
                .transform_edit_state
                .update(&self.state.layer.transform_state);

            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(10.0, 5.0);
                ui.style_mut().spacing.text_edit_width = 80.0;

                ui.label(RichText::new("Position").heading());

                ui.horizontal(|ui| {
                    ui.label("x:");

                    let new_x = ui.text_edit_editable_value_singleline(
                        &mut self.state.layer.transform_edit_state.x,
                    );

                    let current_left = self.state.layer.transform_state.rect.left_top().x;

                    self.state.layer.transform_state.rect = self
                        .state
                        .layer
                        .transform_state
                        .rect
                        .translate(Vec2::new(new_x - current_left, 0.0));

                    ui.label("y:");

                    let new_y = ui.text_edit_editable_value_singleline(
                        &mut self.state.layer.transform_edit_state.y,
                    );

                    let current_top = self.state.layer.transform_state.rect.left_top().y;

                    self.state.layer.transform_state.rect = self
                        .state
                        .layer
                        .transform_state
                        .rect
                        .translate(Vec2::new(0.0, new_y - current_top));
                });

                ui.separator();

                ui.label(RichText::new("Size").heading());

                ui.horizontal(|ui| {
                    ui.label("Width:");

                    let new_width = ui.text_edit_editable_value_singleline(
                        &mut self.state.layer.transform_edit_state.width,
                    );

                    self.state.layer.transform_state.rect.set_width(new_width);

                    ui.label("Height:");

                    let new_height = ui.text_edit_editable_value_singleline(
                        &mut self.state.layer.transform_edit_state.height,
                    );

                    self.state.layer.transform_state.rect.set_height(new_height);
                });

                ui.separator();

                ui.label(RichText::new("Rotation").heading());

                ui.horizontal(|ui| {
                    ui.label("Degrees:");

                    let new_rotation = ui.text_edit_editable_value_singleline(
                        &mut self.state.layer.transform_edit_state.rotation,
                    );

                    self.state.layer.transform_state.rotation = new_rotation.to_radians();
                });
            });

            ui.set_enabled(true);
        });
    }
}
