use std::usize;

use eframe::egui::{self};
use egui::{Pos2, Rect, Sense, Slider, Vec2};

use egui_extras::Column;
use indexmap::IndexMap;
use strum::IntoEnumIterator;

use crate::{
    layout::{grid_layout::{GridDistribution, GridLayout}, stack_layout::{StackCrossAxisAlignment, StackLayout, StackLayoutDirection, StackLayoutDistribution}, LayoutItem, Margin},
    model::page::{Page},
    scene::canvas_scene::{CanvasHistoryKind, CanvasHistoryManager},
    utils::EguiUiExt,
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
                let stack_items: Vec<LayoutItem> = canvas_state.into();
                let grid_layout = GridLayout::new(page_size.x, page_size.y, gap, margin);
                grid_layout.layout(&stack_items)
            }
            Layout::CenteredWeightedGridLayout => {
                let stack_items: Vec<LayoutItem> = canvas_state.into();
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

                let items: Vec<LayoutItem> = canvas_state.into();
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

                let items: Vec<LayoutItem> = canvas_state.into();
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

        if n >= 3 {
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

impl From<&mut CanvasState> for Vec<LayoutItem> {
    fn from(state: &mut CanvasState) -> Vec<LayoutItem> {
        state
            .quick_layout_order
            .iter()
            .filter_map(|layer_id| {
                let layer = state.layers.get(layer_id).unwrap();
                if let LayerContent::Photo(photo) = &layer.content {
                    let cropped_width =
                        photo.photo.metadata.rotated_width() as f32 * photo.crop.width();
                    let cropped_height =
                        photo.photo.metadata.rotated_height() as f32 * photo.crop.height();
                    Some(LayoutItem {
                        aspect_ratio: cropped_width / cropped_height,
                        id: *layer_id,
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}
