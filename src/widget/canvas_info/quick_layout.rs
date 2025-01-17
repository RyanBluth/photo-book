use eframe::egui::{self};
use egui::{Pos2, Rect, Sense, Vec2};

use egui_extras::Column;
use indexmap::IndexMap;
use strum::IntoEnumIterator;

use crate::{
    id::LayerId,
    model::page::Page,
    scene::canvas_scene::{CanvasHistoryKind, CanvasHistoryManager},
    utils::EguiUiExt,
    widget::{
        page_canvas::{Canvas, CanvasState},
        spacer::Spacer,
    },
};

use super::layers::Layer;

#[derive(Debug, Clone, Copy)]
struct QuickLayoutRegion {
    absolute_rect: Rect,
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
        let regions = match self {
            Layout::GridLayout { n, padding } => {
                let grid_size = (*n as f32).sqrt().ceil() as usize;
                let cell_size = 1.0 / grid_size as f32;
                canvas_state
                    .quick_layout_order
                    .iter()
                    .enumerate()
                    .map(|(index, layer_id)| {
                        let layer = canvas_state.layers.get(layer_id).unwrap();
                        let row = index / grid_size;
                        let col = index % grid_size;
                        let rect = Rect::from_min_size(
                            Pos2::new(
                                col as f32 * cell_size + padding,
                                row as f32 * cell_size + padding,
                            ),
                            Vec2::new(cell_size - 2.0 * padding, cell_size - 2.0 * padding),
                        );
                        QuickLayoutRegion {
                            absolute_rect: QuickLayout::fractional_rect_for_layer_in_page(
                                layer,
                                &canvas_state.page.value,
                                rect,
                                QuickLayoutFillMode::Fill,
                            ),
                        }
                    })
                    .collect::<Vec<_>>()
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

                        QuickLayoutRegion {
                            absolute_rect: QuickLayout::fractional_rect_for_layer_in_page(
                                layer,
                                &canvas_state.page.value,
                                rect,
                                QuickLayoutFillMode::Fill,
                            ),
                        }
                    })
                    .collect::<Vec<_>>()
            }
            Layout::HighlightLayout { padding } => {
                let n = canvas_state.quick_layout_order.len();
                let mut regions = vec![];
                let highlight_layer_index = 0;

                let highlight_region = QuickLayoutRegion {
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
                        regions.push(highlight_region.clone());
                        continue;
                    }
                    regions.push(QuickLayoutRegion {
                        absolute_rect: QuickLayout::fractional_rect_for_layer_in_page(
                            canvas_state.layers.get(layer_id).unwrap(),
                            &canvas_state.page.value,
                            Rect::from_min_size(
                                Pos2::new(0.6, min_y + non_highlight_count as f32 * photo_height),
                                Vec2::new(0.4, photo_height),
                            ),
                            QuickLayoutFillMode::Margin(*padding),
                        ),
                    });

                    non_highlight_count += 1;
                }

                regions
            }
            Layout::VerticalStackLayout => {
                let n = canvas_state.quick_layout_order.len();
                let margin = 0.02;
                let available_height = 1.0 - margin * (n as f32 + 1.0);
                let cell_height = available_height / n as f32;
                canvas_state
                    .quick_layout_order
                    .iter()
                    .enumerate()
                    .map(|(i, layer_id)| {
                        let layer = canvas_state.layers.get(layer_id).unwrap();
                        let y = margin * (i as f32 + 1.0) + cell_height * i as f32;
                        QuickLayoutRegion {
                            absolute_rect: QuickLayout::fractional_rect_for_layer_in_page(
                                layer,
                                &canvas_state.page.value,
                                Rect::from_min_size(
                                    Pos2::new(margin, y),
                                    Vec2::new(1.0 - 2.0 * margin, cell_height),
                                ),
                                QuickLayoutFillMode::Fill,
                            ),
                        }
                    })
                    .collect::<Vec<_>>()
            }
            Layout::HorizontalStackLayout => {
                let n = canvas_state.quick_layout_order.len();
                let margin = 0.02;
                let available_width = 1.0 - margin * (n as f32 + 1.0);
                let cell_width = available_width / n as f32;
                canvas_state
                    .quick_layout_order
                    .iter()
                    .enumerate()
                    .map(|(i, layer_id)| {
                        let layer = canvas_state.layers.get(layer_id).unwrap();
                        let x = margin * (i as f32 + 1.0) + cell_width * i as f32;
                        QuickLayoutRegion {
                            absolute_rect: QuickLayout::fractional_rect_for_layer_in_page(
                                layer,
                                &canvas_state.page.value,
                                Rect::from_min_size(
                                    Pos2::new(x, margin),
                                    Vec2::new(cell_width, 1.0 - 2.0 * margin),
                                ),
                                QuickLayoutFillMode::Fill,
                            ),
                        }
                    })
                    .collect::<Vec<_>>()
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
                        QuickLayoutRegion {
                            absolute_rect: QuickLayout::fractional_rect_for_layer_in_page(
                                layer,
                                &canvas_state.page.value,
                                Rect::from_min_size(Pos2::new(x, y), Vec2::new(size, size)),
                                QuickLayoutFillMode::Fill,
                            ),
                        }
                    })
                    .collect::<Vec<_>>()
            }
        };

        for (index, layer_id) in canvas_state.quick_layout_order.iter().enumerate() {
            canvas_state
                .layers
                .get_mut(layer_id)
                .unwrap()
                .transform_state
                .rect = regions[index].absolute_rect;
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
        let column_width = 256.0;
        let row_height = 256.0;
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
                            let page_rect = ui.max_rect();
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
            layouts.push(Layout::GridLayout { n, padding: 0.0 });
            layouts.push(Layout::GridLayout { n, padding: 0.05 });
            layouts.push(Layout::GridLayout { n, padding: 0.1 });
            layouts.push(Layout::GridLayout { n, padding: 0.2 });
            layouts.push(Layout::GridLayout { n, padding: 0.3 });
        } else if n == 2 {
            layouts.push(Layout::CenteredWeightedGridLayout { n, padding: 0.02 });
            layouts.push(Layout::HighlightLayout { padding: 0.2 });
            layouts.push(Layout::HighlightLayout { padding: 0.1 });
            layouts.push(Layout::VerticalStackLayout);
            layouts.push(Layout::HorizontalStackLayout);
        } else if n >= 3 {
            layouts.push(Layout::CenteredWeightedGridLayout { n, padding: 0.0 });
            layouts.push(Layout::CenteredWeightedGridLayout { n, padding: 0.02 });
            layouts.push(Layout::CenteredWeightedGridLayout { n, padding: 0.1 });

            layouts.push(Layout::HighlightLayout { padding: 0.0 });
            layouts.push(Layout::HighlightLayout { padding: 0.1 });
            layouts.push(Layout::VerticalStackLayout);
            layouts.push(Layout::HorizontalStackLayout);
            layouts.push(Layout::ZigzagLayout);

            layouts.push(Layout::GridLayout { n, padding: 0.0 });
            layouts.push(Layout::GridLayout { n, padding: 0.025 });
            layouts.push(Layout::GridLayout { n, padding: 0.05 });
            layouts.push(Layout::GridLayout { n, padding: 0.1 });
        }

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
