use std::{fmt::Display, str::FromStr};

use eframe::{
    egui::{self, FontTweak, Grid, RichText, Ui},
    epaint::{FontFamily, FontId, Vec2},
};

use crate::{utils::EditableValueTextEdit, widget::page_canvas::TransformableState};

use super::layers::{EditableValue, Layer};

pub struct TextControlState<'a> {
    layer: &'a mut Layer,
}

impl<'a> TextControlState<'a> {
    pub fn new(layer: &'a mut Layer) -> Self {
        Self { layer }
    }
}

pub struct TextControl<'a> {
    state: TextControlState<'a>,
}

impl<'a> TextControl<'a> {
    pub fn new(state: TextControlState<'a>) -> Self {
        Self { state }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let response = ui.allocate_ui(ui.available_size(), |ui| match self.state.layer.content {
            super::layers::LayerContent::Photo(_) => {
                ui.label("No text layer selected");
            }
            super::layers::LayerContent::Text(_) => {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(10.0, 5.0);
                    ui.style_mut().spacing.text_edit_width = 80.0;

                    ui.label(RichText::new("Text").heading());

                    ui.horizontal(|ui| {
                        let text = &mut self.state.layer.content;
                        match text {
                            super::layers::LayerContent::Photo(_) => (),
                            super::layers::LayerContent::Text(text) => {
                                let mut new_text = text.text.clone();
                                ui.label("Text:");
                                ui.text_edit_singleline(&mut new_text);
                                text.text = new_text;
                            }
                        }
                    });
                });
            }
        });
    }
}
