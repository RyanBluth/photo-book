use std::fmt::{Display, Formatter};

use eframe::egui::{self};
use egui::{Pos2, RichText, Vec2};

use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::{model, utils::RectExt};

use super::layers::Layer;

#[derive(Debug, PartialEq)]
pub struct ScaleModeState<'a> {
    scale_mode: &'a mut model::scale_mode::ScaleMode,
}

impl ScaleModeState<'_> {
    pub fn new(scale_mode: &mut model::scale_mode::ScaleMode) -> ScaleModeState<'_> {
        ScaleModeState { scale_mode }
    }
}

#[derive(Debug, PartialEq)]
pub struct ScaleMode<'a> {
    pub state: &'a mut ScaleModeState<'a>,
}

impl<'a> ScaleMode<'a> {
    pub fn new(state: &'a mut ScaleModeState<'a>) -> ScaleMode<'a> {
        ScaleMode { state }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 5.0);

            ui.heading("Scale Mode");

            ui.horizontal(|ui| {
                for scale_mode in model::scale_mode::ScaleMode::iter() {
                    let is_selected = *self.state.scale_mode == scale_mode;
                    if ui
                        .selectable_label(is_selected, format!("{}", scale_mode))
                        .clicked()
                    {
                        *self.state.scale_mode = scale_mode;
                    }
                }
            });
        });
    }
}
