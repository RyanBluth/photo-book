use eframe::egui::{Rect, Vec2};
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
    x: f32,
    y: f32,
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
            x: 0.0,
            y: 0.0,
        }
    }

    pub fn with_distribution(mut self, distribution: GridDistribution) -> Self {
        self.distribution = distribution;
        self
    }

    pub fn with_x(mut self, x: f32) -> Self {
        self.x = x;
        self
    }

    pub fn with_y(mut self, y: f32) -> Self {
        self.y = y;
        self
    }

    pub fn layout(&self, items: &[LayoutItem]) -> IndexMap<usize, Rect> {
        if items.is_empty() {
            return IndexMap::new();
        }

        match self.direction {
            StackLayoutDirection::Vertical => self.layout_vertical(items),
            StackLayoutDirection::Horizontal => self.layout_horizontal(items),
        }
    }

    fn layout_vertical(&self, items: &[LayoutItem]) -> IndexMap<usize, Rect> {
        let num_rows_target_per_column =
            (items.len() as f32).sqrt().ceil().max(1.0) as usize;
        let columns: Vec<&[LayoutItem]> = items
            .chunks(num_rows_target_per_column)
            .collect();

        if columns.is_empty() {
            return IndexMap::new();
        }
        let num_columns = columns.len();

        let total_gaps_width = self.gap * (num_columns.saturating_sub(1) as f32);
        let available_width =
            self.width - self.margin.left - self.margin.right - total_gaps_width;
        let column_width = (available_width / num_columns as f32).max(0.0);
        let stack_height = self.height - self.margin.top - self.margin.bottom;

        match self.distribution {
            GridDistribution::Equal => columns
                .iter()
                .enumerate()
                .flat_map(|(col_idx, column_items)| {
                    StackLayout {
                        width: column_width,
                        height: stack_height,
                        gap: self.gap,
                        margin: Margin::none(),
                        direction: StackLayoutDirection::Vertical,
                        alignment: StackCrossAxisAlignment::Center,
                        distribution: StackLayoutDistribution::Grid,
                        x: self.x + self.margin.left + col_idx as f32 * (column_width + self.gap),
                        y: self.y + self.margin.top,
                    }
                    .layout(column_items)
                })
                .collect(),
            GridDistribution::CenterWeighted => {
                let item_dimensions_per_column: Vec<IndexMap<usize, Vec2>> =
                    columns
                        .iter()
                        .map(|column_items| {
                            StackLayout::calculate_vertical_item_dimensions(
                                column_width,
                                stack_height,
                                self.gap,
                                Margin::none(),
                                column_items,
                            )
                        })
                        .collect();

                let max_rows = item_dimensions_per_column
                    .iter()
                    .map(|dim_map| dim_map.len())
                    .max()
                    .unwrap_or(0);

                let mut common_row_heights = vec![f32::MAX; max_rows];
                for dim_map in &item_dimensions_per_column {
                    for (row_idx, size) in dim_map.values().enumerate() {
                        if row_idx < common_row_heights.len() {
                            common_row_heights[row_idx] =
                                common_row_heights[row_idx].min(size.y);
                        }
                    }
                }
                common_row_heights.retain(|&h| h != f32::MAX && h > 0.0);

                let total_rows_content_height: f32 = common_row_heights.iter().sum();
                let total_rows_gaps_height =
                    (common_row_heights.len().saturating_sub(1)) as f32 * self.gap;
                let total_grid_block_height =
                    total_rows_content_height + total_rows_gaps_height;

                let vertical_offset =
                    (stack_height - total_grid_block_height).max(0.0) / 2.0;

                columns
                    .iter()
                    .enumerate()
                    .flat_map(|(col_idx, column_items)| {
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
                            x: self.x + self.margin.left
                                + col_idx as f32 * (column_width + self.gap),
                            y: self.y + self.margin.top + vertical_offset,
                        }
                        .layout(column_items)
                    })
                    .collect()
            }
        }
    }

    fn layout_horizontal(&self, items: &[LayoutItem]) -> IndexMap<usize, Rect> {
        let num_cols_target_per_row = (items.len() as f32).sqrt().ceil().max(1.0) as usize;
        let num_rows = (items.len() as f32 / num_cols_target_per_row as f32)
            .ceil()
            .max(1.0) as usize;

        let mut temp_rows: Vec<Vec<LayoutItem>> = vec![Vec::new(); num_rows];
        for (idx, item) in items.iter().enumerate() {
            temp_rows[idx % num_rows].push(item.clone());
        }

        let rows: Vec<Vec<LayoutItem>> = temp_rows
            .into_iter()
            .filter(|r| !r.is_empty())
            .collect();

        if rows.is_empty() {
            return IndexMap::new();
        }
        let actual_num_rows = rows.len();

        let total_gaps_height = self.gap * (actual_num_rows.saturating_sub(1) as f32);
        let available_height =
            self.height - self.margin.top - self.margin.bottom - total_gaps_height;
        let row_height = (available_height / actual_num_rows as f32).max(0.0);
        let stack_width = self.width - self.margin.left - self.margin.right;

        match self.distribution {
            GridDistribution::Equal => rows
                .iter()
                .enumerate()
                .flat_map(|(row_idx, row_items)| {
                    StackLayout {
                        width: stack_width,
                        height: row_height,
                        gap: self.gap,
                        margin: Margin::none(),
                        direction: StackLayoutDirection::Horizontal,
                        alignment: StackCrossAxisAlignment::Center,
                        distribution: StackLayoutDistribution::Grid,
                        x: self.x + self.margin.left,
                        y: self.y + self.margin.top + row_idx as f32 * (row_height + self.gap),
                    }
                    .layout(row_items.as_slice())
                })
                .collect(),
            GridDistribution::CenterWeighted => {
                let item_dimensions_per_row: Vec<IndexMap<usize, Vec2>> =
                    rows
                        .iter()
                        .map(|row_items| {
                            StackLayout::calculate_horizontal_item_dimensions(
                                stack_width,
                                row_height,
                                self.gap,
                                Margin::none(),
                                row_items.as_slice(),
                            )
                        })
                        .collect();

                // Compute common column widths (min width per column across rows)
                let max_columns = item_dimensions_per_row
                    .iter()
                    .map(|dim_map| dim_map.len())
                    .max()
                    .unwrap_or(0);

                let mut common_column_widths = vec![f32::MAX; max_columns];
                for dim_map in &item_dimensions_per_row {
                    for (col_idx, size) in dim_map.values().enumerate() {
                        if col_idx < common_column_widths.len() {
                            common_column_widths[col_idx] =
                                common_column_widths[col_idx].min(size.x);
                        }
                    }
                }
                common_column_widths.retain(|&w| w != f32::MAX && w > 0.0);

                let total_cols_content_width: f32 = common_column_widths.iter().sum();
                let total_cols_gaps_width =
                    (common_column_widths.len().saturating_sub(1)) as f32 * self.gap;
                let total_grid_block_width =
                    total_cols_content_width + total_cols_gaps_width;

                let horizontal_offset =
                    (stack_width - total_grid_block_width).max(0.0) / 2.0;

                // Compute actual content height per row (max item height in each row)
                let row_content_heights: Vec<f32> = item_dimensions_per_row
                    .iter()
                    .map(|dim_map| {
                        dim_map
                            .values()
                            .map(|size| size.y)
                            .fold(0.0, f32::max)
                    })
                    .collect();

                let total_rows_content_height: f32 = row_content_heights.iter().sum();
                let total_rows_gaps_height =
                    (row_content_heights.len().saturating_sub(1)) as f32 * self.gap;
                let total_grid_block_height =
                    total_rows_content_height + total_rows_gaps_height;
                let full_height = self.height - self.margin.top - self.margin.bottom;
                let vertical_offset =
                    (full_height - total_grid_block_height).max(0.0) / 2.0;

                let mut current_y = self.y + self.margin.top + vertical_offset;
                rows
                    .iter()
                    .enumerate()
                    .flat_map(|(row_idx, row_items)| {
                        let this_row_height = row_content_heights[row_idx];
                        let y_pos = current_y;
                        current_y += this_row_height + self.gap;
                        StackLayout {
                            width: stack_width,
                            height: this_row_height,
                            gap: self.gap,
                            margin: Margin::none(),
                            direction: StackLayoutDirection::Horizontal,
                            alignment: StackCrossAxisAlignment::Center,
                            distribution: StackLayoutDistribution::CenterWeightedGrid {
                                main_axis_sizes: common_column_widths.clone(),
                            },
                            x: self.x + self.margin.left + horizontal_offset,
                            y: y_pos,
                        }
                        .layout(row_items.as_slice())
                    })
                    .collect()
            }
        }
    }
}
