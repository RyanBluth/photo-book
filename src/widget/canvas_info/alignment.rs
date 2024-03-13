use std::fmt::{Display, Formatter};

use eframe::egui::{self};
use egui::{Pos2, RichText, Vec2};

use strum::IntoEnumIterator;
use strum_macros::{EnumIter};

use crate::{
    utils::{RectExt},
};

use super::layers::Layer;

#[derive(Debug, PartialEq)]
pub struct AlignmentInfoState<'a> {
    page_size: Vec2,
    layers: Vec<&'a mut Layer>,
}

impl AlignmentInfoState<'_> {
    pub fn new(page_size: Vec2, layers: Vec<&mut Layer>) -> AlignmentInfoState {
        AlignmentInfoState { page_size, layers }
    }
}

#[derive(Debug, PartialEq, EnumIter)]
enum Aligment {
    Left,
    CenterHorizontal,
    CenterVertical,
    Right,
    Top,
    Bottom,
}

impl Display for Aligment {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Aligment::Left => write!(f, "Left"),
            Aligment::CenterHorizontal => write!(f, "Center Horizontal"),
            Aligment::CenterVertical => write!(f, "Center Vertical"),
            Aligment::Right => write!(f, "Right"),
            Aligment::Top => write!(f, "Top"),
            Aligment::Bottom => write!(f, "Bottom"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct AlignmentInfo<'a> {
    pub state: &'a mut AlignmentInfoState<'a>,
}

impl<'a> AlignmentInfo<'a> {
    pub fn new(state: &'a mut AlignmentInfoState<'a>) -> AlignmentInfo<'a> {
        AlignmentInfo { state }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 5.0);

            ui.label(RichText::new("Alignment").heading());

            ui.horizontal(|ui| {
                let aligment_actions = Aligment::iter().filter_map(|alignment| {
                    ui.button(alignment.to_string())
                        .clicked()
                        .then(|| alignment)
                });

                for alignment in aligment_actions {
                    if self.state.layers.len() == 1 {
                        let layer = self.state.layers.first_mut().unwrap();
                        // Align within the page
                        match alignment {
                            Aligment::Left => {
                                layer.transform_state.rect = layer
                                    .transform_state
                                    .rect
                                    .translate(Vec2::new(-layer.transform_state.rect.left(), 0.0));
                            }
                            Aligment::CenterHorizontal => {
                                layer.transform_state.rect.set_center(Pos2::new(
                                    self.state.page_size.x / 2.0,
                                    layer.transform_state.rect.center().y,
                                ));
                            }
                            Aligment::CenterVertical => {
                                layer.transform_state.rect.set_center(Pos2::new(
                                    layer.transform_state.rect.center().x,
                                    self.state.page_size.y / 2.0,
                                ));
                            }
                            Aligment::Right => {
                                layer.transform_state.rect =
                                    layer.transform_state.rect.translate(Vec2::new(
                                        self.state.page_size.x - layer.transform_state.rect.right(),
                                        0.0,
                                    ));
                            }
                            Aligment::Top => {
                                layer.transform_state.rect = layer
                                    .transform_state
                                    .rect
                                    .translate(Vec2::new(0.0, -layer.transform_state.rect.top()))
                            }
                            Aligment::Bottom => {
                                layer.transform_state.rect =
                                    layer.transform_state.rect.translate(Vec2::new(
                                        0.0,
                                        self.state.page_size.y
                                            - layer.transform_state.rect.bottom(),
                                    ));
                            }
                        }
                    } else if self.state.layers.len() > 1 {
                        // Align within the selection
                        let mut min_x = f32::MAX;
                        let mut max_x = f32::MIN;
                        let mut min_y = f32::MAX;
                        let mut max_y = f32::MIN;

                        for layer in &mut self.state.layers {
                            min_x = min_x.min(layer.transform_state.rect.left());
                            max_x = max_x.max(layer.transform_state.rect.right());
                            min_y = min_y.min(layer.transform_state.rect.top());
                            max_y = max_y.max(layer.transform_state.rect.bottom());
                        }

                        match alignment {
                            Aligment::Left => {
                                for layer in &mut self.state.layers {
                                    layer.transform_state.rect = layer
                                        .transform_state
                                        .rect
                                        .translate_left_to(min_x);
                                }
                            }
                            Aligment::CenterHorizontal => {
                                let center_x = min_x + (max_x - min_x) / 2.0;
                                for layer in &mut self.state.layers {
                                    layer.transform_state.rect.set_center(Pos2::new(
                                        center_x,
                                        layer.transform_state.rect.center().y,
                                    ));
                                }
                            }
                            Aligment::CenterVertical => {
                                let center_y = min_y + (max_y - min_y) / 2.0;
                                for layer in &mut self.state.layers {
                                    layer.transform_state.rect.set_center(Pos2::new(
                                        layer.transform_state.rect.center().x,
                                        center_y,
                                    ));
                                }
                            }
                            Aligment::Right => {
                                for layer in &mut self.state.layers {
                                    layer.transform_state.rect = layer
                                        .transform_state
                                        .rect
                                        .translate_right_to(max_x);
                                }
                            }
                            Aligment::Top => {
                                for layer in &mut self.state.layers {
                                    layer.transform_state.rect = layer
                                        .transform_state
                                        .rect
                                        .translate_top_to(min_y);
                                }
                            }
                            Aligment::Bottom => {
                                for layer in &mut self.state.layers {
                                    layer.transform_state.rect = layer
                                        .transform_state
                                        .rect
                                        .translate_bottom_to(max_y);
                                }
                            }
                        }
                    }
                }
            });

            ui.separator();
        });
    }
}
