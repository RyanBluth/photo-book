use std::usize;

use eframe::egui::{self};
use egui::{Rect, Vec2};

use indexmap::IndexMap;
use strum::IntoEnumIterator;


use super::{stack_layout::{StackCrossAxisAlignment, StackLayout, StackLayoutDirection, StackLayoutDistribution}, Margin, LayoutItem};


#[derive(Debug, Clone, Copy)]
pub enum GridDistribution {
    Equal,
    CenterWeighted,
}

#[derive(Debug, Clone)]
pub struct GridLayout {
    width: f32,
    height: f32,
    gap: f32,
    margin: Margin,
    distribution: GridDistribution,
}

impl GridLayout {
    pub fn new(width: f32, height: f32, gap: f32, margin: f32) -> Self {
        Self {
            width,
            height,
            gap,
            margin: Margin::all(margin),
            distribution: GridDistribution::Equal,
        }
    }

    pub fn with_distribution(mut self, distribution: GridDistribution) -> Self {
        self.distribution = distribution;
        self
    }

    pub fn layout(&self, items: &[LayoutItem]) -> IndexMap<usize, Rect> {
        let grid_size = (items.len() as f32).sqrt().ceil() as usize;
        let column_size =
            (self.width - self.margin.left - (self.margin.right - self.gap)) / grid_size as f32;

        let column_items = items
            .chunks(grid_size)
            .filter(|items| !items.is_empty())
            .collect::<Vec<_>>();

        let grid_size = column_items.len();

        match self.distribution {
            GridDistribution::Equal => column_items
                .iter()
                .enumerate()
                .map(|(column, items)| {
                    StackLayout {
                        width: column_size,
                        height: self.height,
                        gap: self.gap,
                        margin: Margin {
                            top: self.margin.top,
                            right: if column != items.len() - 1 {
                                self.gap
                            } else {
                                0.0
                            },
                            bottom: self.margin.bottom,
                            left: 0.0,
                        },
                        direction: StackLayoutDirection::Vertical,
                        alignment: StackCrossAxisAlignment::Center,
                        distribution: StackLayoutDistribution::Grid,
                        x: column as f32 * column_size + self.margin.left,
                        y: 0.0,
                    }
                    .layout(items)
                })
                .flatten()
                .collect::<IndexMap<usize, Rect>>(),
            GridDistribution::CenterWeighted => {
                // Calculate item dimensions for each column
                let item_dimensions: Vec<IndexMap<usize, Vec2>> = column_items
                    .iter()
                    .map(|items| {
                        StackLayout::calculate_vertical_item_dimensions(
                            column_size,
                            self.height,
                            self.gap,
                            Margin {
                                top: self.margin.top,
                                right: 0.0,
                                bottom: self.margin.bottom,
                                left: self.gap,
                            },
                            items,
                        )
                    })
                    .collect();

                // Find minimum heights for each row across all columns
                let main_axis_sizes: Vec<f32> = item_dimensions.iter().enumerate().fold(
                    Vec::<f32>::new(),
                    |mut acc: Vec<f32>, (idx, column)| {
                        for (col_idx, size) in column.values().enumerate() {
                            if let Some(existing_size) = acc.get_mut(col_idx) {
                                *existing_size = existing_size.min(size.y);
                            } else {
                                acc.push(size.y);
                            }
                        }
                        acc
                    },
                );

                // Calculate total height needed for all rows + gaps
                let total_height = main_axis_sizes.iter().sum::<f32>()
                    + (main_axis_sizes.len().saturating_sub(1)) as f32 * self.gap;

                // Calculate vertical offset to center the grid
                let vertical_offset =
                    (self.height - (self.margin.top + self.margin.bottom) - total_height) / 2.0;
                let vertical_offset = vertical_offset.max(0.0);

                let total_width = column_size * grid_size as f32
                    + self.gap * (grid_size - 1) as f32
                    + self.margin.left
                    + self.margin.right;

                let horizontal_offset = (self.width - total_width) / 2.0;
                let horizontal_offset = horizontal_offset.max(0.0);

                column_items
                    .iter()
                    .enumerate()
                    .map(|(index, items)| {
                        StackLayout {
                            width: column_size,
                            height: self.height,
                            gap: self.gap,
                            margin: Margin {
                                top: self.margin.top + vertical_offset,
                                right: self.gap,
                                bottom: self.margin.bottom,
                                left: 0.0,
                            },
                            direction: StackLayoutDirection::Vertical,
                            alignment: StackCrossAxisAlignment::Center,
                            distribution: StackLayoutDistribution::CenterWeightedGrid {
                                main_axis_sizes: main_axis_sizes.clone(),
                            },
                            x: index as f32 * column_size + horizontal_offset + self.margin.left,
                            y: 0.0,
                        }
                        .layout(items)
                    })
                    .flatten()
                    .collect::<IndexMap<usize, Rect>>()
            }
        }
    }
}
