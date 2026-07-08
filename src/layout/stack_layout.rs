use eframe::egui::{Pos2, Rect, Vec2};
use indexmap::IndexMap;

use super::{LayoutItem, Margin, fit_aspect_ratio_in_rect};

#[derive(Debug, Clone)]
pub enum StackLayoutDirection {
    Vertical,
    Horizontal,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum StackCrossAxisAlignment {
    Start,
    Center,
    End,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum StackLayoutDistribution {
    Start,
    Center,
    End,
    EqualSpacing,
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

#[derive(Debug, Clone, Copy)]
enum Axis {
    Vertical,
    Horizontal,
}

impl Axis {
    fn main_size(self, size: Vec2) -> f32 {
        match self {
            Axis::Vertical => size.y,
            Axis::Horizontal => size.x,
        }
    }

    fn cross_size(self, size: Vec2) -> f32 {
        match self {
            Axis::Vertical => size.x,
            Axis::Horizontal => size.y,
        }
    }

    fn pos(self, main: f32, cross: f32) -> Pos2 {
        match self {
            Axis::Vertical => Pos2::new(cross, main),
            Axis::Horizontal => Pos2::new(main, cross),
        }
    }

    fn size(self, main: f32, cross: f32) -> Vec2 {
        match self {
            Axis::Vertical => Vec2::new(cross, main),
            Axis::Horizontal => Vec2::new(main, cross),
        }
    }

    fn rect(self, main: f32, cross: f32, main_size: f32, cross_size: f32) -> Rect {
        Rect::from_min_size(self.pos(main, cross), self.size(main_size, cross_size))
    }

    fn translate(self, main: f32, cross: f32) -> Vec2 {
        match self {
            Axis::Vertical => Vec2::new(cross, main),
            Axis::Horizontal => Vec2::new(main, cross),
        }
    }

    fn main_min(self, rect: Rect) -> f32 {
        match self {
            Axis::Vertical => rect.min.y,
            Axis::Horizontal => rect.min.x,
        }
    }

    fn with_cross(self, rect: Rect, cross: f32) -> Rect {
        self.rect(
            self.main_min(rect),
            cross,
            self.main_size(rect.size()),
            self.cross_size(rect.size()),
        )
    }
}

impl StackLayout {
    pub fn layout(&self, items: &[LayoutItem]) -> IndexMap<usize, Rect> {
        if items.is_empty() {
            return IndexMap::new();
        }

        match self.direction {
            StackLayoutDirection::Vertical => self.layout_axis(items, Axis::Vertical),
            StackLayoutDirection::Horizontal => self.layout_axis(items, Axis::Horizontal),
        }
    }

    fn layout_axis(&self, items: &[LayoutItem], axis: Axis) -> IndexMap<usize, Rect> {
        let item_dimensions = match axis {
            Axis::Vertical => StackLayout::calculate_vertical_item_dimensions(
                self.width,
                self.height,
                self.gap,
                self.margin,
                items,
            ),
            Axis::Horizontal => StackLayout::calculate_horizontal_item_dimensions(
                self.width,
                self.height,
                self.gap,
                self.margin,
                items,
            ),
        };

        let total_gap = self.gap * items.len().saturating_sub(1) as f32;
        let main_less_margin = match axis {
            Axis::Vertical => self.height - (self.margin.top + self.margin.bottom),
            Axis::Horizontal => self.width - (self.margin.left + self.margin.right),
        };
        let cross_less_margin = match axis {
            Axis::Vertical => self.width - (self.margin.left + self.margin.right),
            Axis::Horizontal => self.height - (self.margin.top + self.margin.bottom),
        };
        let total_scaled_main = item_dimensions
            .values()
            .map(|size| axis.main_size(*size))
            .sum::<f32>()
            + total_gap;

        let top_left_rects: IndexMap<usize, Rect> = {
            let mut main_offset = 0.0;
            item_dimensions
                .iter()
                .map(|(id, size)| {
                    let rect = axis.rect(
                        main_offset,
                        0.0,
                        axis.main_size(*size),
                        axis.cross_size(*size),
                    );
                    main_offset += axis.main_size(*size) + self.gap;
                    (*id, rect)
                })
                .collect()
        };

        let distributed: IndexMap<usize, Rect> = match &self.distribution {
            StackLayoutDistribution::Start => top_left_rects,
            StackLayoutDistribution::Center => {
                let main_diff = (main_less_margin - total_scaled_main) / 2.0;
                top_left_rects
                    .iter()
                    .map(|(id, rect)| (*id, rect.translate(axis.translate(main_diff, 0.0))))
                    .collect()
            }
            StackLayoutDistribution::End => {
                let main_diff = main_less_margin - total_scaled_main;
                top_left_rects
                    .iter()
                    .map(|(id, rect)| (*id, rect.translate(axis.translate(main_diff, 0.0))))
                    .collect()
            }
            StackLayoutDistribution::EqualSpacing => {
                let total_item_main = item_dimensions
                    .values()
                    .map(|size| axis.main_size(*size))
                    .sum::<f32>();
                let remaining_space = main_less_margin - total_item_main;
                let equal_spacing = (remaining_space / (items.len() as f32 + 1.0)).max(self.gap);

                let mut main_offset = equal_spacing;
                item_dimensions
                    .iter()
                    .map(|(id, size)| {
                        let rect = axis.rect(
                            main_offset,
                            0.0,
                            axis.main_size(*size),
                            axis.cross_size(*size),
                        );
                        main_offset += axis.main_size(*size) + equal_spacing;
                        (*id, rect)
                    })
                    .collect()
            }
            StackLayoutDistribution::Grid => {
                let cell_size = (main_less_margin - total_gap) / items.len() as f32;
                let mut main_offset = 0.0;
                item_dimensions
                    .iter()
                    .map(|(id, size)| {
                        let target_rect = axis.rect(main_offset, 0.0, cell_size, cross_less_margin);
                        let fitted_rect = fit_aspect_ratio_in_rect(size.x / size.y, target_rect);
                        main_offset += cell_size + self.gap;
                        (*id, fitted_rect)
                    })
                    .collect()
            }
            StackLayoutDistribution::CenterWeightedGrid { main_axis_sizes } => {
                let mut main_offset = 0.0;
                item_dimensions
                    .iter()
                    .enumerate()
                    .map(|(idx, (id, size))| {
                        let main_size = main_axis_sizes
                            .get(idx)
                            .copied()
                            .unwrap_or_else(|| axis.main_size(*size));
                        let target_rect = axis.rect(main_offset, 0.0, main_size, cross_less_margin);
                        let fitted_rect = fit_aspect_ratio_in_rect(size.x / size.y, target_rect);
                        main_offset += main_size + self.gap;
                        (*id, fitted_rect)
                    })
                    .collect()
            }
        };

        let aligned: IndexMap<usize, Rect> = match self.alignment {
            StackCrossAxisAlignment::Start => distributed,
            StackCrossAxisAlignment::Center => distributed
                .iter()
                .map(|(id, rect)| {
                    let cross = (cross_less_margin - axis.cross_size(rect.size())) / 2.0;
                    (*id, axis.with_cross(*rect, cross))
                })
                .collect(),
            StackCrossAxisAlignment::End => distributed
                .iter()
                .map(|(id, rect)| {
                    let cross = cross_less_margin - axis.cross_size(rect.size());
                    (*id, axis.with_cross(*rect, cross))
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
