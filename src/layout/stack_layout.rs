use std::usize;

use eframe::egui::{self};
use egui::{Pos2, Rect, Vec2};

use indexmap::IndexMap;

use crate::utils::RectExt;

use super::{LayoutItem, Margin};

#[derive(Debug, Clone)]
pub enum StackLayoutDirection {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone)]
pub enum StackCrossAxisAlignment {
    _Start,
    Center,
    _End,
}

#[derive(Debug, Clone)]
pub enum StackLayoutDistribution {
    _Start,
    Center,
    _End,
    _EqualSpacing,
    Grid,
    CenterWeightedGrid { main_axis_sizes: Vec<f32> },
}

#[derive(Debug, Clone)]
pub struct StackLayout {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
    pub gap: f32,
    pub margin: Margin,
    pub direction: StackLayoutDirection,
    pub alignment: StackCrossAxisAlignment,
    pub distribution: StackLayoutDistribution,
}

impl StackLayout {
    pub fn layout(&self, items: &[LayoutItem]) -> IndexMap<usize, Rect> {
        match self.direction {
            StackLayoutDirection::Vertical => self.layout_vertical(items),
            StackLayoutDirection::Horizontal => self.layout_horizontal(items),
        }
    }

    fn layout_vertical(&self, items: &[LayoutItem]) -> IndexMap<usize, Rect> {
        let item_dimensions = StackLayout::calculate_vertical_item_dimensions(
            self.width,
            self.height,
            self.gap,
            self.margin,
            items,
        );

        let total_gap: f32 = self.gap * (items.len() as f32 - 1.0);
        let height_less_margin = self.height - (self.margin.top + self.margin.bottom);
        let width_less_margin = self.width - (self.margin.left + self.margin.right);

        let total_scaled_height =
            item_dimensions.values().map(|dim| dim.y).sum::<f32>() + total_gap;

        let top_left_rects = {
            let mut y_offset = 0.0;
            item_dimensions
                .iter()
                .map(|(id, size)| {
                    let rect = Rect::from_min_size(Pos2::new(0.0, y_offset), *size);
                    y_offset += size.y + self.gap;
                    (*id, rect)
                })
                .collect()
        };

        let distributed: IndexMap<usize, Rect> = match &self.distribution {
            StackLayoutDistribution::_Start => top_left_rects,
            StackLayoutDistribution::Center => {
                let height_diff = height_less_margin - total_scaled_height;
                top_left_rects
                    .iter()
                    .map(|(id, rect)| {
                        (
                            *id,
                            Rect::from_min_size(
                                Pos2::new(rect.min.x, rect.min.y + height_diff),
                                rect.size(),
                            ),
                        )
                    })
                    .collect()
            }
            StackLayoutDistribution::_End => {
                let height_diff = height_less_margin - total_scaled_height;
                top_left_rects
                    .iter()
                    .map(|(id, rect)| {
                        (
                            *id,
                            Rect::from_min_size(
                                Pos2::new(rect.min.x, rect.min.y + height_diff),
                                rect.size(),
                            ),
                        )
                    })
                    .collect()
            }
            StackLayoutDistribution::_EqualSpacing => {
                let total_item_height = item_dimensions.values().map(|dim| dim.y).sum::<f32>();
                let remaining_space = height_less_margin - total_item_height;
                let equal_spacing = (remaining_space / (items.len() as f32 + 1.0)).max(self.gap);

                let mut y_offset = equal_spacing;
                item_dimensions
                    .iter()
                    .map(|(id, size)| {
                        let rect = Rect::from_min_size(Pos2::new(0.0, y_offset), *size);
                        y_offset += size.y + equal_spacing;
                        (*id, rect)
                    })
                    .collect()
            }
            StackLayoutDistribution::Grid => {
                let cell_size = (height_less_margin - total_gap) / items.len() as f32;
                let mut y_offset = 0.0;

                item_dimensions
                    .iter()
                    .map(|(id, size)| {
                        let rect = Rect::from_min_size(Pos2::new(0.0, y_offset), *size);
                        let target_rect = Rect::from_min_size(
                            Pos2::new(0.0, y_offset),
                            Vec2::new(width_less_margin, cell_size),
                        );
                        let fitted_rect = rect.fit_and_center_within(target_rect);
                        y_offset += cell_size + self.gap;
                        (*id, fitted_rect)
                    })
                    .collect()
            }
            StackLayoutDistribution::CenterWeightedGrid { main_axis_sizes } => {
                let _total_item_height = main_axis_sizes.iter().sum::<f32>();
                let mut y_offset = 0.0; //(height_less_margin - total_item_height) / 2.0;
                item_dimensions
                    .iter()
                    .enumerate()
                    .map(|(idx, (id, size))| {
                        let rect = Rect::from_min_size(Pos2::new(0.0, y_offset), *size);
                        let target_rect = Rect::from_min_size(
                            Pos2::new(0.0, y_offset),
                            Vec2::new(width_less_margin, main_axis_sizes[idx]),
                        );
                        let fitted_rect = rect.fit_and_center_within(target_rect);
                        y_offset += main_axis_sizes[idx] + self.gap;
                        (*id, fitted_rect)
                    })
                    .collect()
            }
        };

        let aligned = match self.alignment {
            StackCrossAxisAlignment::_Start => distributed,
            StackCrossAxisAlignment::Center => distributed
                .iter()
                .map(|(id, rect)| {
                    let x = (width_less_margin - rect.width()) / 2.0;
                    let rect = Rect::from_min_size(Pos2::new(x, rect.min.y), rect.size());
                    (*id, rect)
                })
                .collect(),
            StackCrossAxisAlignment::_End => distributed
                .iter()
                .map(|(id, rect)| {
                    let x = width_less_margin - rect.width();
                    let rect = Rect::from_min_size(Pos2::new(x, rect.min.y), rect.size());
                    (*id, rect)
                })
                .collect(),
        };

        aligned
            .iter()
            .map(|(id, rect)| {
                (
                    *id,
                    rect.translate(Vec2::new(
                        self.margin.left + self.x,
                        self.margin.top + self.y,
                    )),
                )
            })
            .collect()
    }

    fn layout_horizontal(&self, items: &[LayoutItem]) -> IndexMap<usize, Rect> {
        let item_dimensions = StackLayout::calculate_horizontal_item_dimensions(
            self.width,
            self.height,
            self.gap,
            self.margin,
            items,
        );

        let total_gap: f32 = self.gap * (items.len() as f32 - 1.0);
        let width_less_margin = self.width - (self.margin.left + self.margin.right);
        let height_less_margin = self.height - (self.margin.top + self.margin.bottom);
        let total_scaled_width = item_dimensions.values().map(|dim| dim.x).sum::<f32>() + total_gap;

        let top_left_rects: IndexMap<usize, Rect> = {
            let mut x_offset = 0.0;
            item_dimensions
                .iter()
                .map(|(id, size)| {
                    let rect = Rect::from_min_size(Pos2::new(x_offset, 0.0), *size);
                    x_offset += size.x + self.gap;
                    (*id, rect)
                })
                .collect()
        };

        let distributed: IndexMap<usize, Rect> = match &self.distribution {
            StackLayoutDistribution::_Start => top_left_rects,
            StackLayoutDistribution::Center => {
                let width_diff = (width_less_margin - total_scaled_width) / 2.0;
                top_left_rects
                    .iter()
                    .map(|(id, rect)| {
                        let rect = Rect::from_min_size(
                            Pos2::new(rect.min.x + width_diff, rect.min.y),
                            rect.size(),
                        );
                        (*id, rect)
                    })
                    .collect()
            }
            StackLayoutDistribution::_End => {
                let width_diff = width_less_margin - total_scaled_width;
                top_left_rects
                    .iter()
                    .map(|(id, rect)| {
                        let rect = Rect::from_min_size(
                            Pos2::new(rect.min.x + width_diff, rect.min.y),
                            rect.size(),
                        );
                        (*id, rect)
                    })
                    .collect()
            }
            StackLayoutDistribution::_EqualSpacing => {
                let total_item_width = item_dimensions.values().map(|dim| dim.x).sum::<f32>();
                let remaining_space = width_less_margin - total_item_width;
                let equal_spacing = (remaining_space / (items.len() as f32 + 1.0)).max(self.gap);

                let mut x_offset = equal_spacing;
                item_dimensions
                    .iter()
                    .map(|(id, size)| {
                        let rect = Rect::from_min_size(Pos2::new(x_offset, 0.0), *size);
                        x_offset += size.x + equal_spacing;
                        (*id, rect)
                    })
                    .collect()
            }
            StackLayoutDistribution::Grid => {
                let cell_size = (width_less_margin - total_gap) / items.len() as f32;
                let mut x_offset = 0.0;
                item_dimensions
                    .iter()
                    .map(|(id, size)| {
                        let rect = Rect::from_min_size(Pos2::new(x_offset, 0.0), *size);
                        let fitted_rect = rect.fit_and_center_within(Rect::from_min_size(
                            Pos2::new(x_offset, 0.0),
                            Vec2::new(cell_size, height_less_margin),
                        ));
                        x_offset += cell_size + self.gap;
                        (*id, fitted_rect)
                    })
                    .collect()
            }
            StackLayoutDistribution::CenterWeightedGrid { main_axis_sizes } => {
                let mut x_offset = 0.0;
                item_dimensions
                    .iter()
                    .enumerate()
                    .map(|(idx, (id, size))| {
                        let rect = Rect::from_min_size(Pos2::new(x_offset, 0.0), *size);
                        let target_rect = Rect::from_min_size(
                            Pos2::new(x_offset, 0.0),
                            Vec2::new(main_axis_sizes[idx], height_less_margin),
                        );
                        let fitted_rect = rect.fit_and_center_within(target_rect);
                        x_offset += main_axis_sizes[idx] + self.gap;
                        (*id, fitted_rect)
                    })
                    .collect()
            }
        };

        let aligned = match self.alignment {
            StackCrossAxisAlignment::_Start => distributed,
            StackCrossAxisAlignment::Center => distributed
                .iter()
                .map(|(id, rect)| {
                    let y: f32 = (self.height - self.margin.top - self.margin.bottom) / 2.0
                        - rect.height() / 2.0;
                    let rect = Rect::from_min_size(Pos2::new(rect.min.x, y), rect.size());
                    (*id, rect)
                })
                .collect(),
            StackCrossAxisAlignment::_End => distributed
                .iter()
                .map(|(id, rect)| {
                    let y = self.height - rect.height();
                    let rect = Rect::from_min_size(Pos2::new(rect.min.x, y), rect.size());
                    (*id, rect)
                })
                .collect(),
        };

        aligned
            .iter()
            .map(|(id, rect)| {
                let rect = rect.translate(Vec2::new(
                    self.margin.left + self.x,
                    self.margin.top + self.y,
                ));
                (*id, rect)
            })
            .collect()
    }

    pub fn calculate_horizontal_item_dimensions(
        width: f32,
        height: f32,
        gap: f32,
        margin: Margin,
        items: &[LayoutItem],
    ) -> IndexMap<usize, Vec2> {
        let mut item_dimensions: IndexMap<usize, Vec2> = items
            .iter()
            .map(|item: &LayoutItem| {
                let height: f32 = height - (margin.top + margin.bottom);
                let width = height * item.aspect_ratio;
                (item.id, Vec2::new(width, height))
            })
            .collect();

        let total_items_width = item_dimensions.values().map(|dim| dim.x).sum::<f32>();
        let total_gap: f32 = gap * (items.len() as f32 - 1.0);
        let total_width = total_items_width + total_gap;
        let max_height = item_dimensions
            .values()
            .map(|dim| dim.y)
            .fold(0.0, f32::max);
        let width_less_margin = width - (margin.left + margin.right);
        let height_less_margin = height - (margin.top + margin.bottom);

        if total_width > width_less_margin || max_height > height_less_margin {
            let item_width_scale = width_less_margin / total_items_width
                - (gap * (items.len() as f32 - 1.0) / total_width);
            let item_height_scale = height_less_margin / max_height;
            let final_scale = item_width_scale.min(item_height_scale);

            item_dimensions.values_mut().for_each(|size| {
                *size *= final_scale;
                size.x = size.x.floor();
                size.y = size.y.floor();
            });
        }

        item_dimensions
    }

    pub fn calculate_vertical_item_dimensions(
        width: f32,
        height: f32,
        gap: f32,
        margin: Margin,
        items: &[LayoutItem],
    ) -> IndexMap<usize, Vec2> {
        let mut item_dimensions: IndexMap<usize, Vec2> = items
            .iter()
            .map(|item: &LayoutItem| {
                let width: f32 = width - (margin.left + margin.right);
                let height = width / item.aspect_ratio;
                (item.id, Vec2::new(width, height))
            })
            .collect();

        let total_items_height = item_dimensions.values().map(|dim| dim.y).sum::<f32>();
        let total_gap: f32 = gap * (items.len() as f32 - 1.0);
        let total_height = total_items_height + total_gap;
        let max_width = item_dimensions
            .values()
            .map(|dim| dim.x)
            .fold(0.0, f32::max);

        let width_less_margin = width - (margin.left + margin.right);
        let height_less_margin = height - (margin.top + margin.bottom);

        if total_height > height || max_width > width_less_margin {
            let item_height_scale = height_less_margin / total_items_height
                - (gap * (items.len() as f32 - 1.0) / total_height);
            let item_width_scale = width_less_margin / max_width;
            let final_scale = item_height_scale.min(item_width_scale);
            item_dimensions.values_mut().for_each(|size| {
                *size *= final_scale;
                size.x = size.x.floor();
                size.y = size.y.floor();
            });
        }

        item_dimensions
    }
}
