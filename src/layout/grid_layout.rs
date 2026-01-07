use std::usize;

use eframe::egui::{self};
use egui::{Rect, Vec2};

use indexmap::IndexMap;

use super::{
    LayoutItem, Margin,
    stack_layout::{
        StackCrossAxisAlignment, StackLayout, StackLayoutDirection, StackLayoutDistribution,
    },
};

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
    direction: StackLayoutDirection,
}

impl GridLayout {
    pub fn new(
        width: f32,
        height: f32,
        gap: f32,
        margin: f32,
        direction: StackLayoutDirection,
    ) -> Self {
        Self {
            width,
            height,
            gap,
            margin: Margin::all(margin),
            distribution: GridDistribution::Equal,
            direction,
        }
    }

    pub fn with_distribution(mut self, distribution: GridDistribution) -> Self {
        self.distribution = distribution;
        self
    }

    pub fn _with_direction(mut self, direction: StackLayoutDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn layout(&self, items: &[LayoutItem]) -> IndexMap<usize, Rect> {
        if items.is_empty() {
            return IndexMap::new();
        }

        match self.direction {
            StackLayoutDirection::Vertical => {
                let num_rows_target_per_column =
                    (items.len() as f32).sqrt().ceil().max(1.0) as usize;
                let columns_definitions: Vec<&[LayoutItem]> = items
                    .chunks(num_rows_target_per_column)
                    .filter(|chunk| !chunk.is_empty())
                    .collect::<Vec<_>>();

                if columns_definitions.is_empty() {
                    return IndexMap::new();
                } // Should not happen if items not empty
                let num_columns = columns_definitions.len();

                let total_gaps_width = self.gap * (num_columns.saturating_sub(1) as f32);
                let available_width_for_columns =
                    self.width - self.margin.left - self.margin.right - total_gaps_width;
                let column_width = (available_width_for_columns / num_columns as f32).max(0.0);
                let stack_height = self.height - self.margin.top - self.margin.bottom;

                match self.distribution {
                    GridDistribution::Equal => columns_definitions
                        .iter()
                        .enumerate()
                        .map(|(col_idx, column_items_slice)| {
                            StackLayout {
                                width: column_width,
                                height: stack_height,
                                gap: self.gap,
                                margin: Margin::none(),
                                direction: StackLayoutDirection::Vertical,
                                alignment: StackCrossAxisAlignment::Center,
                                distribution: StackLayoutDistribution::Grid,
                                x: self.margin.left + col_idx as f32 * (column_width + self.gap),
                                y: self.margin.top,
                            }
                            .layout(column_items_slice)
                        })
                        .flatten()
                        .collect::<IndexMap<usize, Rect>>(),
                    GridDistribution::CenterWeighted => {
                        let item_dimensions_per_column: Vec<IndexMap<usize, Vec2>> =
                            columns_definitions
                                .iter()
                                .map(|column_items_slice| {
                                    StackLayout::calculate_vertical_item_dimensions(
                                        column_width,
                                        stack_height,
                                        self.gap,
                                        Margin::none(),
                                        column_items_slice,
                                    )
                                })
                                .collect();

                        let mut common_row_heights: Vec<f32> = Vec::new();
                        if !columns_definitions.is_empty() {
                            let max_rows = item_dimensions_per_column
                                .iter()
                                .map(|dim_map| dim_map.len())
                                .max()
                                .unwrap_or(0);
                            if max_rows > 0 {
                                common_row_heights = vec![f32::MAX; max_rows];
                                for dim_map in &item_dimensions_per_column {
                                    for (row_idx, size) in dim_map.values().enumerate() {
                                        if row_idx < common_row_heights.len() {
                                            common_row_heights[row_idx] =
                                                common_row_heights[row_idx].min(size.y);
                                        }
                                    }
                                }
                                common_row_heights.retain(|&h| h != f32::MAX && h > 0.0);
                            }
                        }

                        let total_rows_content_height = common_row_heights.iter().sum::<f32>();
                        let total_rows_gaps_height =
                            (common_row_heights.len().saturating_sub(1)) as f32 * self.gap;
                        let total_grid_block_height =
                            total_rows_content_height + total_rows_gaps_height;

                        let vertical_offset_for_grid_block =
                            ((stack_height) - total_grid_block_height).max(0.0) / 2.0;

                        columns_definitions
                            .iter()
                            .enumerate()
                            .map(|(col_idx, column_items_slice)| {
                                StackLayout {
                                    width: column_width,
                                    height: stack_height,
                                    gap: self.gap,
                                    margin: Margin::none(),
                                    direction: StackLayoutDirection::Vertical,
                                    alignment: StackCrossAxisAlignment::Center,
                                    distribution: StackLayoutDistribution::CenterWeightedGrid {
                                        main_axis_sizes: common_row_heights.clone(),
                                    },
                                    x: self.margin.left
                                        + col_idx as f32 * (column_width + self.gap),
                                    y: self.margin.top + vertical_offset_for_grid_block,
                                }
                                .layout(column_items_slice)
                            })
                            .flatten()
                            .collect::<IndexMap<usize, Rect>>()
                    }
                }
            }
            StackLayoutDirection::Horizontal => {
                let num_cols_target_per_row = (items.len() as f32).sqrt().ceil().max(1.0) as usize;
                let num_rows = (items.len() as f32 / num_cols_target_per_row as f32)
                    .ceil()
                    .max(1.0) as usize;

                let mut temp_rows_storage: Vec<Vec<LayoutItem>> = vec![Vec::new(); num_rows];
                for (idx, item_ref) in items.iter().enumerate() {
                    temp_rows_storage[idx % num_rows].push(item_ref.clone());
                }

                let rows_as_vecs_of_items: Vec<Vec<LayoutItem>> = temp_rows_storage
                    .into_iter()
                    .filter(|r_vec| !r_vec.is_empty())
                    .collect();

                if rows_as_vecs_of_items.is_empty() {
                    return IndexMap::new();
                }
                let actual_num_rows = rows_as_vecs_of_items.len();

                let total_gaps_height = self.gap * (actual_num_rows.saturating_sub(1) as f32);
                let available_height_for_rows =
                    self.height - self.margin.top - self.margin.bottom - total_gaps_height;
                let row_height = (available_height_for_rows / actual_num_rows as f32).max(0.0);
                let stack_width = self.width - self.margin.left - self.margin.right;

                match self.distribution {
                    GridDistribution::Equal => rows_as_vecs_of_items
                        .iter()
                        .enumerate()
                        .map(|(row_idx, current_row_vec_ref)| {
                            StackLayout {
                                width: stack_width,
                                height: row_height,
                                gap: self.gap,
                                margin: Margin::none(),
                                direction: StackLayoutDirection::Horizontal,
                                alignment: StackCrossAxisAlignment::Center,
                                distribution: StackLayoutDistribution::Grid,
                                x: self.margin.left,
                                y: self.margin.top + row_idx as f32 * (row_height + self.gap),
                            }
                            .layout(current_row_vec_ref.as_slice())
                        })
                        .flatten()
                        .collect::<IndexMap<usize, Rect>>(),
                    GridDistribution::CenterWeighted => {
                        let item_dimensions_per_row: Vec<IndexMap<usize, Vec2>> =
                            rows_as_vecs_of_items
                                .iter()
                                .map(|current_row_vec_ref| {
                                    StackLayout::calculate_horizontal_item_dimensions(
                                        stack_width,
                                        row_height,
                                        self.gap,
                                        Margin::none(),
                                        current_row_vec_ref.as_slice(),
                                    )
                                })
                                .collect();

                        let mut common_column_widths: Vec<f32> = Vec::new();
                        if !rows_as_vecs_of_items.is_empty() {
                            let max_columns = item_dimensions_per_row
                                .iter()
                                .map(|dim_map| dim_map.len())
                                .max()
                                .unwrap_or(0);
                            if max_columns > 0 {
                                common_column_widths = vec![f32::MAX; max_columns];
                                for dim_map in &item_dimensions_per_row {
                                    for (col_idx, size) in dim_map.values().enumerate() {
                                        if col_idx < common_column_widths.len() {
                                            common_column_widths[col_idx] =
                                                common_column_widths[col_idx].min(size.x);
                                        }
                                    }
                                }
                                common_column_widths.retain(|&w| w != f32::MAX && w > 0.0);
                            }
                        }

                        let total_cols_content_width = common_column_widths.iter().sum::<f32>();
                        let total_cols_gaps_width =
                            (common_column_widths.len().saturating_sub(1)) as f32 * self.gap;
                        let total_grid_block_width =
                            total_cols_content_width + total_cols_gaps_width;

                        let horizontal_offset_for_grid_block =
                            ((stack_width) - total_grid_block_width).max(0.0) / 2.0;

                        rows_as_vecs_of_items
                            .iter()
                            .enumerate()
                            .map(|(row_idx, current_row_vec_ref)| {
                                StackLayout {
                                    width: stack_width,
                                    height: row_height,
                                    gap: self.gap,
                                    margin: Margin::none(),
                                    direction: StackLayoutDirection::Horizontal,
                                    alignment: StackCrossAxisAlignment::Center,
                                    distribution: StackLayoutDistribution::CenterWeightedGrid {
                                        main_axis_sizes: common_column_widths.clone(),
                                    },
                                    x: self.margin.left + horizontal_offset_for_grid_block,
                                    y: self.margin.top + row_idx as f32 * (row_height + self.gap),
                                }
                                .layout(current_row_vec_ref.as_slice())
                            })
                            .flatten()
                            .collect::<IndexMap<usize, Rect>>()
                    }
                }
            }
        }
    }
}
