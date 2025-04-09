use std::{collections::HashMap, usize};

use eframe::egui::{self};
use egui::{Pos2, Rect, Sense, Slider, Vec2};

use egui_extras::Column;
use exif::In;
use indexmap::IndexMap;
use strum::IntoEnumIterator;

use crate::{
    history, main,
    model::page::{self, Page},
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

#[derive(Debug, PartialEq, Clone)]
pub struct QuickLayoutState {
    gap: f32,
    margin: f32,
    last_layout: Option<Layout>,
}

impl<'a> QuickLayoutState {
    pub fn new() -> QuickLayoutState {
        QuickLayoutState {
            gap: 50.0,
            margin: 100.0,
            last_layout: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Layout {
    GridLayout,
    CenteredWeightedGridLayout,
    HighlightLayout { padding: f32 },
    VerticalStackLayout,
    HorizontalStackLayout,
    ZigzagLayout,
}

impl Layout {
    pub fn apply(&self, canvas_state: &mut CanvasState, gap: f32, margin: f32) {
        let page_size = canvas_state.page.value.size_pixels();

        let regions: IndexMap<usize, Rect> = match self {
            Layout::GridLayout => {
                let mut stack_items: Vec<StackLayoutItem> = canvas_state.into();
                let grid_layout = GridLayout::new(page_size.x, page_size.y, gap, margin);
                grid_layout.layout(&stack_items)
            }
            Layout::CenteredWeightedGridLayout => {
                let mut stack_items: Vec<StackLayoutItem> = canvas_state.into();
                let grid_layout = GridLayout::new(page_size.x, page_size.y, gap, margin)
                    .with_distribution(GridDistribution::CenterWeighted);
                grid_layout.layout(&stack_items)
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
                    gap: gap,
                    margin: Margin {
                        top: margin,
                        right: margin,
                        bottom: margin,
                        left: margin,
                    },
                    direction: StackLayoutDirection::Vertical,
                    alignment: StackCrossAxisAlignment::Center,
                    distribution: StackLayoutDistribution::Center,
                    x: 0.0,
                    y: 0.0,
                };

                let items: Vec<StackLayoutItem> = canvas_state.into();
                stack_layout.layout(&items)
            }
            Layout::HorizontalStackLayout => {
                let stack_layout = StackLayout {
                    width: page_size.x,
                    height: page_size.y,
                    gap: gap,
                    margin: Margin {
                        top: margin,
                        right: margin,
                        bottom: margin,
                        left: margin,
                    },
                    direction: StackLayoutDirection::Horizontal,
                    alignment: StackCrossAxisAlignment::Center,
                    distribution: StackLayoutDistribution::Center,
                    x: 0.0,
                    y: 0.0,
                };

                let items: Vec<StackLayoutItem> = canvas_state.into();
                stack_layout.layout(&items)
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
    pub state: &'a mut QuickLayoutState,
    pub canvas_state: &'a mut CanvasState,
    pub history_manager: &'a mut CanvasHistoryManager,
}

impl<'a> QuickLayout<'a> {
    pub fn new(
        state: &'a mut QuickLayoutState,
        canvas_state: &'a mut CanvasState,
        history_manager: &'a mut CanvasHistoryManager,
    ) -> QuickLayout<'a> {
        QuickLayout {
            state,
            canvas_state,
            history_manager,
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

        ui.vertical(|ui| {
            let mut new_gap = self.state.gap;
            let mut new_margin = self.state.margin;

            ui.horizontal(|ui| {
                ui.label("Gap:");
                ui.add(Slider::new(&mut new_gap, 0.0..=100.0));
            });

            ui.horizontal(|ui| {
                ui.label("Margin:");
                ui.add(Slider::new(&mut new_margin, 0.0..=100.0));
            });

            if let Some(last_layout) = self.state.last_layout {
                if new_gap != self.state.gap || new_margin != self.state.margin {
                    last_layout.apply(self.canvas_state, new_gap, new_margin);
                }
            }

            self.state.gap = new_gap;
            self.state.margin = new_margin;

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

                            let mut canvas_state = self.canvas_state.clone_with_new_widget_ids();

                            layout.apply(&mut canvas_state, self.state.gap, self.state.margin);

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
        });

        if let Some(selected_layout) = selected_layout {
            selected_layout.apply(self.canvas_state, self.state.gap, self.state.margin);
            self.canvas_state.last_quick_layout = Some(selected_layout);
            self.state.last_layout = Some(selected_layout);
            self.history_manager
                .save_history(CanvasHistoryKind::QuickLayout, self.canvas_state);
        }
    }

    fn available_layouts(&self) -> Vec<Layout> {
        let n = self.canvas_state.quick_layout_order.len();

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

            layouts.push(Layout::GridLayout);
            layouts.push(Layout::CenteredWeightedGridLayout);
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

#[derive(Debug, Clone)]
pub enum StackLayoutDirection {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone)]
pub enum StackCrossAxisAlignment {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone)]
pub enum StackLayoutDistribution {
    Start,
    Center,
    End,
    EqualSpacing,
    Grid,
    CenterWeightedGrid { main_axis_sizes: Vec<f32> },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Margin {
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
}

impl Margin {
    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StackLayoutItem {
    aspect_ratio: f32,
    id: usize,
}

#[derive(Debug, Clone)]
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
    pub fn layout(&self, items: &[StackLayoutItem]) -> IndexMap<usize, Rect> {
        match self.direction {
            StackLayoutDirection::Vertical => self.layout_vertical(items),
            StackLayoutDirection::Horizontal => self.layout_horizontal(items),
        }
    }

    fn layout_vertical(&self, items: &[StackLayoutItem]) -> IndexMap<usize, Rect> {
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
                let total_item_height = main_axis_sizes.iter().sum::<f32>();
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
            StackLayoutDistribution::CenterWeightedGrid {
                main_axis_sizes: cell_height,
            } => {
                todo!()
            }
        };

        let aligned = match self.alignment {
            StackCrossAxisAlignment::Start => distributed,
            StackCrossAxisAlignment::Center => distributed
                .iter()
                .map(|(id, rect)| {
                    let y: f32 = (self.height - self.margin.top - self.margin.bottom) / 2.0
                        - rect.height() / 2.0;
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
        width: f32,
        height: f32,
        gap: f32,
        margin: Margin,
        items: &[StackLayoutItem],
    ) -> IndexMap<usize, Vec2> {
        let mut item_dimensions: IndexMap<usize, Vec2> = items
            .iter()
            .map(|item: &StackLayoutItem| {
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
        items: &[StackLayoutItem],
    ) -> IndexMap<usize, Vec2> {
        let mut item_dimensions: IndexMap<usize, Vec2> = items
            .iter()
            .map(|item: &StackLayoutItem| {
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
            let item_height_scale =
                height_less_margin / total_items_height - (gap * (items.len() as f32 - 1.0) / total_height);
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

impl From<&mut CanvasState> for Vec<StackLayoutItem> {
    fn from(state: &mut CanvasState) -> Vec<StackLayoutItem> {
        state
            .quick_layout_order
            .iter()
            .filter_map(|layer_id| {
                let layer = state.layers.get(layer_id).unwrap();
                if let LayerContent::Photo(photo) = &layer.content {
                    Some(StackLayoutItem {
                        aspect_ratio: photo.photo.aspect_ratio(),
                        id: *layer_id,
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

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

    pub fn layout(&self, items: &[StackLayoutItem]) -> IndexMap<usize, Rect> {
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
