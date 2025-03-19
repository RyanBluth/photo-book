use std::collections::HashMap;

use eframe::egui::{self};
use egui::{Pos2, Rect, Sense, Vec2};

use egui_extras::Column;
use exif::In;
use indexmap::IndexMap;
use strum::IntoEnumIterator;

use crate::{
    model::page::Page,
    photo,
    scene::canvas_scene::{CanvasHistoryKind, CanvasHistoryManager},
    utils::{EguiUiExt, RectExt},
    widget::{
        canvas::{Canvas, CanvasState},
        spacer::Spacer,
    },
};

use super::layers::{Layer, LayerContent};

#[derive(Debug, Clone, Copy)]
struct QuickLayoutRegion {
    absolute_rect: Rect,
    id: usize,
}

#[derive(Debug, Clone, Copy)]
enum QuickLayoutFillMode {
    Fill,
    Margin(f32),
}

#[derive(Debug, PartialEq)]
pub struct QuickLayoutState<'a> {
    canvas_state: &'a mut CanvasState,
    history_manager: &'a mut CanvasHistoryManager,
}

impl<'a> QuickLayoutState<'a> {
    pub fn new(
        canvas_state: &'a mut CanvasState,
        history_manager: &'a mut CanvasHistoryManager,
    ) -> QuickLayoutState<'a> {
        QuickLayoutState {
            canvas_state,
            history_manager,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Layout {
    GridLayout { n: usize, padding: f32 },
    CenteredWeightedGridLayout { n: usize, padding: f32 },
    HighlightLayout { padding: f32 },
    VerticalStackLayout,
    HorizontalStackLayout,
    ZigzagLayout,
}

impl Layout {
    pub fn apply(&self, canvas_state: &mut CanvasState) {
        let page_size = canvas_state.page.value.size_pixels();

        let regions: IndexMap<usize, Rect> = match self {
            Layout::GridLayout { n, padding } => {
                let grid_size = (*n as f32).sqrt().ceil() as usize;
                let column_size = page_size.x / grid_size as f32;

                let stack_items: Vec<StackLayoutItem> = canvas_state.into();

                (0..grid_size)
                    .map(|column| {
                        let stack_items = stack_items
                            .iter()
                            .skip(column)
                            .step_by(grid_size)
                            .map(|item| item.clone())
                            .collect::<Vec<_>>();

                        StackLayout {
                            width: column_size,
                            height: page_size.y,
                            gap: 50.0,
                            margin: Margin {
                                top: 50.0,
                                right: 50.0,
                                bottom: 50.0,
                                left: if column == 0 { 50.0 } else { 0.0 },
                            },
                            direction: StackLayoutDirection::Vertical,
                            alignment: StackCrossAxisAlignment::Center,
                            distribution: StackLayoutDistribution::Grid,
                            x: column as f32 * column_size,
                            y: 0.0,
                        }
                        .layout(stack_items)
                    })
                    .flatten()
                    .collect::<IndexMap<usize, Rect>>()
            }
            Layout::CenteredWeightedGridLayout { n, padding } => {
                let grid_size = (*n as f32).sqrt().ceil() as usize;
                let fixed_spacing = 0.02; // 2% of page size between images
                let rows = ((n + grid_size - 1) / grid_size) as f32;

                // Calculate cell sizes based on both width and height constraints
                let inner_width = 1.0 - (2.0 * padding);
                let inner_height = 1.0 - (2.0 * padding);

                let width_based_cell_size =
                    (inner_width - (fixed_spacing * (grid_size - 1) as f32)) / grid_size as f32;
                let height_based_cell_size = (inner_height - (fixed_spacing * (rows - 1.0))) / rows;

                // Use the smaller cell size to maintain equal spacing
                let cell_size = width_based_cell_size.min(height_based_cell_size);

                // Recalculate total dimensions with final cell size
                let total_width =
                    (cell_size * grid_size as f32) + (fixed_spacing * (grid_size - 1) as f32);
                let total_height = (cell_size * rows) + (fixed_spacing * (rows - 1.0));

                // Center the grid
                let x_offset = (1.0 - total_width) / 2.0;
                let y_offset = (1.0 - total_height) / 2.0;

                canvas_state
                    .quick_layout_order
                    .iter()
                    .enumerate()
                    .map(|(index, layer_id)| {
                        let layer = canvas_state.layers.get(layer_id).unwrap();
                        let row = index / grid_size;
                        let col = index % grid_size;

                        let x = x_offset + (col as f32 * (cell_size + fixed_spacing));
                        let y = y_offset + (row as f32 * (cell_size + fixed_spacing));

                        let rect =
                            Rect::from_min_size(Pos2::new(x, y), Vec2::new(cell_size, cell_size));

                        (
                            *layer_id,
                            QuickLayout::fractional_rect_for_layer_in_page(
                                layer,
                                &canvas_state.page.value,
                                rect,
                                QuickLayoutFillMode::Fill,
                            ),
                        )
                    })
                    .collect::<IndexMap<usize, Rect>>()
            }
            Layout::HighlightLayout { padding } => {
                let n = canvas_state.quick_layout_order.len();
                let mut regions = IndexMap::new();
                let highlight_layer_index = 0;

                let highlight_region = QuickLayoutRegion {
                    id: canvas_state.quick_layout_order[highlight_layer_index],
                    absolute_rect: QuickLayout::fractional_rect_for_layer_in_page(
                        canvas_state
                            .layers
                            .get(&canvas_state.quick_layout_order[highlight_layer_index])
                            .unwrap(),
                        &canvas_state.page.value,
                        Rect::from_min_size(Pos2::ZERO, Vec2::new(0.6, 1.0)),
                        QuickLayoutFillMode::Margin(*padding),
                    ),
                };

                let highlight_rect = highlight_region.absolute_rect;

                let min_y = highlight_rect.min.y / &canvas_state.page.value.size_pixels().y;
                let max_y = highlight_rect.max.y / &canvas_state.page.value.size_pixels().y;

                let photo_height = (max_y - min_y) / (n - 1) as f32;
                let mut non_highlight_count = 0;

                for (i, layer_id) in canvas_state.quick_layout_order.iter().enumerate() {
                    if i == highlight_layer_index {
                        regions.insert(*layer_id, highlight_region.absolute_rect);
                        continue;
                    }
                    regions.insert(
                        *layer_id,
                        QuickLayout::fractional_rect_for_layer_in_page(
                            canvas_state.layers.get(layer_id).unwrap(),
                            &canvas_state.page.value,
                            Rect::from_min_size(
                                Pos2::new(0.6, min_y + non_highlight_count as f32 * photo_height),
                                Vec2::new(0.4, photo_height),
                            ),
                            QuickLayoutFillMode::Margin(*padding),
                        ),
                    );

                    non_highlight_count += 1;
                }

                regions
            }
            Layout::VerticalStackLayout => {
                let stack_layout = StackLayout {
                    width: page_size.x,
                    height: page_size.y,
                    gap: 50.0,
                    margin: Margin {
                        top: 100.0,
                        right: 100.0,
                        bottom: 100.0,
                        left: 100.0,
                    },
                    direction: StackLayoutDirection::Vertical,
                    alignment: StackCrossAxisAlignment::Center,
                    distribution: StackLayoutDistribution::Grid,
                    x: 0.0,
                    y: 0.0,
                };

                stack_layout.layout(canvas_state.into())
            }
            Layout::HorizontalStackLayout => {
                let stack_layout = StackLayout {
                    width: page_size.x,
                    height: page_size.y,
                    gap: 50.0,
                    margin: Margin {
                        top: 100.0,
                        right: 100.0,
                        bottom: 100.0,
                        left: 100.0,
                    },
                    direction: StackLayoutDirection::Horizontal,
                    alignment: StackCrossAxisAlignment::Center,
                    distribution: StackLayoutDistribution::Grid,
                    x: 0.0,
                    y: 0.0,
                };

                stack_layout.layout(canvas_state.into())
            }
            Layout::ZigzagLayout => {
                let size = 0.3;
                let x_positions = [0.1, 0.6];
                canvas_state
                    .quick_layout_order
                    .iter()
                    .enumerate()
                    .map(|(i, layer_id)| {
                        let layer = canvas_state.layers.get(layer_id).unwrap();
                        let x = x_positions[i % 2];
                        let y = 0.1 + 0.2 * i as f32;
                        (
                            *layer_id,
                            QuickLayout::fractional_rect_for_layer_in_page(
                                layer,
                                &canvas_state.page.value,
                                Rect::from_min_size(Pos2::new(x, y), Vec2::new(size, size)),
                                QuickLayoutFillMode::Fill,
                            ),
                        )
                    })
                    .collect::<IndexMap<usize, Rect>>()
            }
        };

        for layer_id in canvas_state.quick_layout_order.iter() {
            canvas_state
                .layers
                .get_mut(layer_id)
                .unwrap()
                .transform_state
                .rect = regions[layer_id];
        }
    }
}

#[derive(PartialEq)]
pub struct QuickLayout<'a> {
    pub state: &'a mut QuickLayoutState<'a>,
    last_layout: Option<Layout>,
}

impl<'a> QuickLayout<'a> {
    pub fn new(state: &'a mut QuickLayoutState<'a>) -> QuickLayout<'a> {
        QuickLayout {
            state,
            last_layout: None,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = Vec2::splat(10.0);

        let available_layouts = self.available_layouts();

        if available_layouts.is_empty() {
            ui.both_centered(|ui| {
                ui.heading("Add photos to view available layouts.");
            });

            return;
        }

        ui.set_clip_rect(ui.available_rect_before_wrap());

        let available_width = ui.available_width();
        let available_height = ui.available_height();
        let column_width: f32 = ui.available_width();
        let row_height = column_width;
        let num_columns: usize = (available_width / column_width).floor() as usize;

        let spacer_width = (available_width
            - ((column_width + ui.spacing().item_spacing.x) * num_columns as f32)
            - 10.0
            - ui.spacing().item_spacing.x)
            .max(0.0);

        let num_rows = available_layouts.len();

        let mut selected_layout: Option<Layout> = None;

        egui_extras::TableBuilder::new(ui)
            .min_scrolled_height(available_height)
            .columns(Column::exact(column_width), num_columns)
            .column(Column::exact(spacer_width))
            .body(|body| {
                body.rows(row_height, num_rows, |mut row| {
                    let offest = row.index() * num_columns;
                    for i in 0..num_columns {
                        if offest + i >= num_rows {
                            break;
                        }

                        let _index = offest + i;
                        let layout = available_layouts.get(offest + i).unwrap();

                        let mut canvas_state = self.state.canvas_state.clone_with_new_widget_ids();

                        layout.apply(&mut canvas_state);

                        row.col(|ui| {
                            let page_rect = ui.max_rect().shrink2(Vec2::new(20.0, 0.0));
                            Canvas::new(
                                &mut canvas_state,
                                page_rect,
                                &mut CanvasHistoryManager::preview(),
                            )
                            .show_preview(ui, page_rect);

                            let click_response = ui.allocate_rect(page_rect, Sense::click());

                            if click_response.clicked() {
                                selected_layout = Some(layout.clone());
                            }
                        });
                    }

                    row.col(|ui| {
                        ui.add(Spacer::new(spacer_width, row_height));
                    });
                })
            });

        if let Some(selected_layout) = selected_layout {
            selected_layout.apply(self.state.canvas_state);
            self.state.canvas_state.last_quick_layout = Some(selected_layout);
            self.state
                .history_manager
                .save_history(CanvasHistoryKind::QuickLayout, self.state.canvas_state);
        }
    }

    fn available_layouts(&self) -> Vec<Layout> {
        let n = self.state.canvas_state.quick_layout_order.len();

        if n == 0 {
            return vec![];
        }

        let mut layouts: Vec<Layout> = vec![];

        if n == 1 {
            // Grid serves as a centering layout for a single photo
            // layouts.push(Layout::GridLayout { n, padding: 0.0 });
            // layouts.push(Layout::GridLayout { n, padding: 0.05 });
            // layouts.push(Layout::GridLayout { n, padding: 0.1 });
            // layouts.push(Layout::GridLayout { n, padding: 0.2 });
            // layouts.push(Layout::GridLayout { n, padding: 0.3 });
        } else if n == 2 {
            // layouts.push(Layout::CenteredWeightedGridLayout { n, padding: 0.02 });
            // layouts.push(Layout::HighlightLayout { padding: 0.2 });
            // layouts.push(Layout::HighlightLayout { padding: 0.1 });
            // layouts.push(Layout::VerticalStackLayout);
            // layouts.push(Layout::HorizontalStackLayout);
        } else if n >= 3 {
            // layouts.push(Layout::CenteredWeightedGridLayout { n, padding: 0.0 });
            // layouts.push(Layout::CenteredWeightedGridLayout { n, padding: 0.02 });
            // layouts.push(Layout::CenteredWeightedGridLayout { n, padding: 0.1 });

            // layouts.push(Layout::HighlightLayout { padding: 0.0 });
            // layouts.push(Layout::HighlightLayout { padding: 0.1 });
            // layouts.push(Layout::VerticalStackLayout);
            // layouts.push(Layout::HorizontalStackLayout);
            // layouts.push(Layout::ZigzagLayout);

            layouts.push(Layout::GridLayout { n, padding: 0.0 });
            layouts.push(Layout::GridLayout { n, padding: 0.025 });
            layouts.push(Layout::GridLayout { n, padding: 0.05 });
            layouts.push(Layout::GridLayout { n, padding: 0.1 });
        }

        layouts.push(Layout::VerticalStackLayout);
        layouts.push(Layout::HorizontalStackLayout);

        layouts
    }

    fn fractional_rect_for_layer_in_page(
        layer: &Layer,
        page: &Page,
        max_rect_percentage: Rect,
        margin_option: QuickLayoutFillMode,
    ) -> Rect {
        let page_size = page.size_pixels();
        let max_rect = Rect::from_min_size(
            egui::Pos2::new(
                max_rect_percentage.min.x * page_size.x,
                max_rect_percentage.min.y * page_size.y,
            ),
            egui::Vec2::new(
                max_rect_percentage.width() * page_size.x,
                max_rect_percentage.height() * page_size.y,
            ),
        );

        let layer_rect = layer.transform_state.rect;

        let layer_aspect_ratio = layer_rect.width() / layer_rect.height();

        let (max_width, max_height) = match margin_option {
            QuickLayoutFillMode::Fill => (max_rect.width(), max_rect.height()),
            QuickLayoutFillMode::Margin(margin_percentage) => (
                max_rect.width() * (1.0 - margin_percentage),
                max_rect.height() * (1.0 - margin_percentage),
            ),
        };

        let (new_width, new_height) = if layer_aspect_ratio > 1.0 {
            let width = max_width;
            let height = width / layer_aspect_ratio;
            if height <= max_height {
                (width, height)
            } else {
                (max_height * layer_aspect_ratio, max_height)
            }
        } else {
            let height = max_height;
            let width = height * layer_aspect_ratio;
            if width <= max_width {
                (width, height)
            } else {
                (max_width, max_width / layer_aspect_ratio)
            }
        };

        let x = max_rect.min.x + (max_rect.width() - new_width) / 2.0;
        let y = max_rect.min.y + (max_rect.height() - new_height) / 2.0;

        Rect::from_min_size(
            egui::Pos2::new(x, y),
            egui::Vec2::new(new_width, new_height),
        )
    }
}

pub enum StackLayoutDirection {
    Vertical,
    Horizontal,
}

pub enum StackCrossAxisAlignment {
    Start,
    Center,
    End,
}

pub enum StackLayoutDistribution {
    Start,
    Center,
    End,
    EqualSpacing,
    Grid,
}

pub struct Margin {
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
}

#[derive(Debug, Clone)]
pub struct StackLayoutItem {
    aspect_ratio: f32,
    id: usize,
}

pub struct StackLayout {
    width: f32,
    height: f32,
    x: f32,
    y: f32,
    gap: f32,
    margin: Margin,
    direction: StackLayoutDirection,
    alignment: StackCrossAxisAlignment,
    distribution: StackLayoutDistribution,
}

impl StackLayout {
    pub fn layout(&self, items: Vec<StackLayoutItem>) -> IndexMap<usize, Rect> {
        match self.direction {
            StackLayoutDirection::Vertical => self.layout_vertical(&items),
            StackLayoutDirection::Horizontal => self.layout_horizontal(&items),
        }
    }

    fn layout_vertical(&self, items: &[StackLayoutItem]) -> IndexMap<usize, Rect> {
        let mut item_dimensions = self.calculate_vertical_item_dimensions(items);
        let total_items_height = item_dimensions.values().map(|dim| dim.y).sum::<f32>();
        let total_gap: f32 = self.gap * (items.len() as f32 - 1.0);
        let total_height = total_items_height + total_gap;
        let max_width = item_dimensions
            .values()
            .map(|dim| dim.x)
            .fold(0.0, f32::max);
        let height_less_margin = self.height - (self.margin.top + self.margin.bottom);
        let width_less_margin = self.width - (self.margin.left + self.margin.right);

        if total_height > self.height || max_width > width_less_margin {
            let item_height_scale = self.height / total_items_height
                - (self.gap * (items.len() as f32 - 1.0) / total_height);
            let item_width_scale = width_less_margin / max_width;
            let final_scale = item_height_scale.min(item_width_scale);
            item_dimensions.values_mut().for_each(|size| {
                *size *= final_scale;
                size.x = size.x.floor();
                size.y = size.y.floor();
            });
        }

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

        let distributed: IndexMap<usize, Rect> = match self.distribution {
            StackLayoutDistribution::Start => top_left_rects,
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
            StackLayoutDistribution::End => {
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
            StackLayoutDistribution::EqualSpacing => {
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
                        let fitted_rect = rect.fit_and_center_within(Rect::from_min_size(
                            Pos2::new(0.0, y_offset),
                            Vec2::new(width_less_margin, cell_size),
                        ));
                        y_offset += cell_size + self.gap;
                        (*id, fitted_rect)
                    })
                    .collect()
            }
        };

        let aligned = match self.alignment {
            StackCrossAxisAlignment::Start => distributed,
            StackCrossAxisAlignment::Center => distributed
                .iter()
                .map(|(id, rect)| {
                    let x = (width_less_margin - rect.width()) / 2.0;
                    let rect = Rect::from_min_size(Pos2::new(x, rect.min.y), rect.size());
                    (*id, rect)
                })
                .collect(),
            StackCrossAxisAlignment::End => distributed
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

    fn layout_horizontal(&self, items: &[StackLayoutItem]) -> IndexMap<usize, Rect> {
        let mut item_dimensions = self.calculate_horizontal_item_dimensions(items);
        let total_items_width = item_dimensions.values().map(|dim| dim.x).sum::<f32>();
        let total_gap: f32 = self.gap * (items.len() as f32 - 1.0);
        let total_width = total_items_width + total_gap;
        let max_height = item_dimensions
            .values()
            .map(|dim| dim.y)
            .fold(0.0, f32::max);
        let width_less_margin = self.width - (self.margin.left + self.margin.right);
        let height_less_margin = self.height - (self.margin.top + self.margin.bottom);

        if total_width > width_less_margin || max_height > height_less_margin {
            let item_width_scale = width_less_margin / total_items_width
                - (self.gap * (items.len() as f32 - 1.0) / total_width);
            let item_height_scale = height_less_margin / max_height;
            let final_scale = item_width_scale.min(item_height_scale);

            item_dimensions.values_mut().for_each(|size| {
                *size *= final_scale;
                size.x = size.x.floor();
                size.y = size.y.floor();
            });
        }

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

        let distributed: IndexMap<usize, Rect> = match self.distribution {
            StackLayoutDistribution::Start => top_left_rects,
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
            StackLayoutDistribution::End => {
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
            StackLayoutDistribution::EqualSpacing => {
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
        };

        let aligned = match self.alignment {
            StackCrossAxisAlignment::Start => distributed,
            StackCrossAxisAlignment::Center => distributed
                .iter()
                .map(|(id, rect)| {
                    let y = (self.height - rect.height()) / 2.0;
                    let rect = Rect::from_min_size(Pos2::new(rect.min.x, y), rect.size());
                    (*id, rect)
                })
                .collect(),
            StackCrossAxisAlignment::End => distributed
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

    fn calculate_horizontal_item_dimensions(
        &self,
        items: &[StackLayoutItem],
    ) -> IndexMap<usize, Vec2> {
        items
            .iter()
            .map(|item: &StackLayoutItem| {
                let height: f32 = self.height - (self.margin.top + self.margin.bottom);
                let width = height * item.aspect_ratio;
                (item.id, Vec2::new(width, height))
            })
            .collect()
    }

    fn calculate_vertical_item_dimensions(
        &self,
        items: &[StackLayoutItem],
    ) -> IndexMap<usize, Vec2> {
        items
            .iter()
            .map(|item: &StackLayoutItem| {
                let width: f32 = self.width - (self.margin.left + self.margin.right);
                let height = width / item.aspect_ratio;
                (item.id, Vec2::new(width, height))
            })
            .collect()
    }
}

impl From<&mut CanvasState> for Vec<StackLayoutItem> {
    fn from(state: &mut CanvasState) -> Vec<StackLayoutItem> {
        state
            .quick_layout_order
            .iter()
            .map(|layer_id| {
                let layer = state.layers.get(layer_id).unwrap();
                if let LayerContent::Photo(photo) = &layer.content {
                    StackLayoutItem {
                        aspect_ratio: photo.photo.aspect_ratio(),
                        id: *layer_id,
                    }
                } else {
                    panic!("Invalid layer content type for quick layout");
                }
            })
            .collect()
    }
}
