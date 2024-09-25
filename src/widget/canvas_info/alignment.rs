use std::fmt::{Display, Formatter};

use eframe::egui::{self};
use egui::{Button, ImageSource, Pos2, RichText, Ui, Vec2};

use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::{assets::Asset, utils::RectExt};

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
enum Alignment {
    Left,
    CenterHorizontal,
    CenterVertical,
    Right,
    Top,
    Bottom,
}

impl Display for Alignment {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Alignment::Left => write!(f, "Left"),
            Alignment::CenterHorizontal => write!(f, "Center Horizontal"),
            Alignment::CenterVertical => write!(f, "Center Vertical"),
            Alignment::Right => write!(f, "Right"),
            Alignment::Top => write!(f, "Top"),
            Alignment::Bottom => write!(f, "Bottom"),
        }
    }
}

impl Alignment {
    fn icon(&self) -> ImageSource {
        match self {
            Alignment::Left => Asset::horizontal_align_left(),
            Alignment::CenterHorizontal => Asset::horizontal_align_center(),
            Alignment::CenterVertical => Asset::vertical_align_center(),
            Alignment::Right => Asset::horizontal_align_right(),
            Alignment::Top => Asset::vertical_align_top(),
            Alignment::Bottom => Asset::vertical_align_bottom(),
        }
    }
}

#[derive(Debug, PartialEq, EnumIter)]
enum Distruibution {
    Horizontal,
    Vertical,
}

impl Display for Distruibution {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Distruibution::Horizontal => write!(f, "Horizontal"),
            Distruibution::Vertical => write!(f, "Vertical"),
        }
    }
}

impl Distruibution {
    fn icon(&self) -> ImageSource {
        match self {
            Distruibution::Horizontal => Asset::distribute_horizontal(),
            Distruibution::Vertical => Asset::distribute_vertical(),
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

            self.alignment(ui);

            self.distribution(ui);

            ui.separator();
        });
    }

    fn distribution(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let distribution_actions = Distruibution::iter().filter_map(|distribution| {
                ui.add(Button::image(distribution.icon()))
                    .on_hover_text(distribution.to_string())
                    .clicked()
                    .then_some(distribution)
            });

            for distribution in distribution_actions {
                if self.state.layers.len() > 1 {
                    match distribution {
                        Distruibution::Horizontal => {
                            let (min, max, width_total) = self.state.layers.iter().fold(
                                (f32::MAX, f32::MIN, 0.0),
                                |(min, max, width_total), layer| {
                                    (
                                        min.min(layer.transform_state.rect.left()),
                                        max.max(layer.transform_state.rect.right()),
                                        width_total + layer.transform_state.rect.width(),
                                    )
                                },
                            );

                            let available_space = max - min - width_total;
                            let space_between =
                                available_space / (self.state.layers.len() - 1) as f32;

                            let mut sorted_indices: Vec<usize> =
                                (0..self.state.layers.len()).collect();

                            sorted_indices.sort_by(|a, b| {
                                self.state.layers[*a]
                                    .transform_state
                                    .rect
                                    .left()
                                    .partial_cmp(&self.state.layers[*b].transform_state.rect.left())
                                    .unwrap()
                            });

                            let mut offset = self.state.layers[sorted_indices[0]]
                                .transform_state
                                .rect
                                .width()
                                + space_between
                                + min;
                            for i in 1..self.state.layers.len() - 1 {
                                let width = self.state.layers[sorted_indices[i]]
                                    .transform_state
                                    .rect
                                    .width();
                                self.state.layers[sorted_indices[i]]
                                    .transform_state
                                    .rect
                                    .set_left(offset);
                                self.state.layers[sorted_indices[i]]
                                    .transform_state
                                    .rect
                                    .set_right(offset + width);
                                offset += self.state.layers[sorted_indices[i]]
                                    .transform_state
                                    .rect
                                    .width()
                                    + space_between;
                            }
                        }
                        Distruibution::Vertical => {
                            let (min, max, height_total) = self.state.layers.iter().fold(
                                (f32::MAX, f32::MIN, 0.0),
                                |(min, max, height_total), layer| {
                                    (
                                        min.min(layer.transform_state.rect.top()),
                                        max.max(layer.transform_state.rect.bottom()),
                                        height_total + layer.transform_state.rect.height(),
                                    )
                                },
                            );

                            let available_space = max - min - height_total;
                            let space_between =
                                available_space / (self.state.layers.len() - 1) as f32;

                            let mut sorted_indices: Vec<usize> =
                                (0..self.state.layers.len()).collect();

                            sorted_indices.sort_by(|a, b| {
                                self.state.layers[*a]
                                    .transform_state
                                    .rect
                                    .top()
                                    .partial_cmp(&self.state.layers[*b].transform_state.rect.top())
                                    .unwrap()
                            });

                            let mut offset = self.state.layers[sorted_indices[0]]
                                .transform_state
                                .rect
                                .height()
                                + space_between
                                + min;
                            for i in 1..self.state.layers.len() - 1 {
                                let height = self.state.layers[sorted_indices[i]]
                                    .transform_state
                                    .rect
                                    .height();
                                self.state.layers[sorted_indices[i]]
                                    .transform_state
                                    .rect
                                    .set_top(offset);
                                self.state.layers[sorted_indices[i]]
                                    .transform_state
                                    .rect
                                    .set_bottom(offset + height);
                                offset += self.state.layers[sorted_indices[i]]
                                    .transform_state
                                    .rect
                                    .height()
                                    + space_between;
                            }
                        }
                    }
                }
            }
        });
    }

    fn alignment(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let alignment_actions = Alignment::iter().filter_map(|alignment| {
                ui.add(Button::image(alignment.icon()))
                    .on_hover_text(alignment.to_string())
                    .clicked()
                    .then_some(alignment)
            });

            for alignment in alignment_actions {
                if self.state.layers.len() == 1 {
                    let layer = self.state.layers.first_mut().unwrap();
                    // Align within the page
                    match alignment {
                        Alignment::Left => {
                            layer.transform_state.rect = layer
                                .transform_state
                                .rect
                                .translate(Vec2::new(-layer.transform_state.rect.left(), 0.0));
                        }
                        Alignment::CenterHorizontal => {
                            layer.transform_state.rect.set_center(Pos2::new(
                                self.state.page_size.x / 2.0,
                                layer.transform_state.rect.center().y,
                            ));
                        }
                        Alignment::CenterVertical => {
                            layer.transform_state.rect.set_center(Pos2::new(
                                layer.transform_state.rect.center().x,
                                self.state.page_size.y / 2.0,
                            ));
                        }
                        Alignment::Right => {
                            layer.transform_state.rect =
                                layer.transform_state.rect.translate(Vec2::new(
                                    self.state.page_size.x - layer.transform_state.rect.right(),
                                    0.0,
                                ));
                        }
                        Alignment::Top => {
                            layer.transform_state.rect = layer
                                .transform_state
                                .rect
                                .translate(Vec2::new(0.0, -layer.transform_state.rect.top()))
                        }
                        Alignment::Bottom => {
                            layer.transform_state.rect =
                                layer.transform_state.rect.translate(Vec2::new(
                                    0.0,
                                    self.state.page_size.y - layer.transform_state.rect.bottom(),
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
                        Alignment::Left => {
                            for layer in &mut self.state.layers {
                                layer.transform_state.rect =
                                    layer.transform_state.rect.translate_left_to(min_x);
                            }
                        }
                        Alignment::CenterHorizontal => {
                            let center_x = min_x + (max_x - min_x) / 2.0;
                            for layer in &mut self.state.layers {
                                layer.transform_state.rect.set_center(Pos2::new(
                                    center_x,
                                    layer.transform_state.rect.center().y,
                                ));
                            }
                        }
                        Alignment::CenterVertical => {
                            let center_y = min_y + (max_y - min_y) / 2.0;
                            for layer in &mut self.state.layers {
                                layer.transform_state.rect.set_center(Pos2::new(
                                    layer.transform_state.rect.center().x,
                                    center_y,
                                ));
                            }
                        }
                        Alignment::Right => {
                            for layer in &mut self.state.layers {
                                layer.transform_state.rect =
                                    layer.transform_state.rect.translate_right_to(max_x);
                            }
                        }
                        Alignment::Top => {
                            for layer in &mut self.state.layers {
                                layer.transform_state.rect =
                                    layer.transform_state.rect.translate_top_to(min_y);
                            }
                        }
                        Alignment::Bottom => {
                            for layer in &mut self.state.layers {
                                layer.transform_state.rect =
                                    layer.transform_state.rect.translate_bottom_to(max_y);
                            }
                        }
                    }
                }
            }
        });
    }
}
