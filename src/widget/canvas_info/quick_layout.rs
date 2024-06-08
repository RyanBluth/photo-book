use eframe::egui::{self};
use egui::{Pos2, Rect, Sense, Vec2};

use egui_extras::Column;
use indexmap::IndexMap;
use strum::IntoEnumIterator;

use crate::{
    id::LayerId,
    model::page::Page,
    scene::canvas_scene::{CanvasHistoryKind, CanvasHistoryManager},
    widget::{
        page_canvas::{Canvas, CanvasState},
        spacer::Spacer,
    },
};

use super::layers::Layer;

struct QukcLayoutRegion {
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

#[derive(Debug, PartialEq)]
pub struct QuickLayout<'a> {
    pub state: &'a mut QuickLayoutState<'a>,
}

impl<'a> QuickLayout<'a> {
    pub fn new(state: &'a mut QuickLayoutState<'a>) -> QuickLayout<'a> {
        QuickLayout { state }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = Vec2::splat(10.0);

        let window_width = ui.available_width();
        let window_height = ui.available_height();
        let column_width = 256.0;
        let row_height = 256.0;
        let num_columns: usize = (window_width / column_width).floor() as usize;

        //let padding_size = num_columns as f32 * 10.0;
        let spacer_width = (window_width
            - ((column_width + ui.spacing().item_spacing.x) * num_columns as f32)
            - 10.0
            - ui.spacing().item_spacing.x)
            .max(0.0);

        let available_layouts = self.available_layouts();
        let num_rows = available_layouts.len();

        let mut selected_layout = None;

        egui_extras::TableBuilder::new(ui)
            .min_scrolled_height(window_height)
            .columns(Column::exact(column_width), num_columns)
            .column(Column::exact(spacer_width))
            .body(|body| {
                body.rows(row_height, num_rows, |mut row| {
                    let offest = row.index() * num_columns;
                    for i in 0..num_columns {
                        if offest + i >= num_rows {
                            break;
                        }

                        let index = offest + i;
                        let layout = available_layouts.get(offest + i).unwrap();

                        let mut canvas_state = self.state.canvas_state.clone_with_new_widget_ids();

                        canvas_state
                            .layers
                            .iter_mut()
                            .filter(|layer| layer.1.content.is_photo())
                            .enumerate()
                            .for_each(|(index, (_, layer))| {
                                layer.transform_state.rect = layout[index].absolute_rect;
                            });

                        row.col(|ui| {
                            let page_rect = ui.max_rect();
                            Canvas::new(
                                &mut canvas_state,
                                page_rect,
                                &mut CanvasHistoryManager::new(),
                            )
                            .show_preview(ui, page_rect);

                            let click_response = ui.allocate_rect(page_rect, Sense::click());

                            if click_response.clicked() {
                                selected_layout = Some(canvas_state.clone());
                            }
                        });
                    }

                    row.col(|ui| {
                        ui.add(Spacer::new(spacer_width, row_height));
                    });
                })
            });

        if let Some(selected_layout) = selected_layout {
            self.state.canvas_state.layers = selected_layout.layers;
            self.state
                .history_manager
                .save_history(CanvasHistoryKind::QuickLayout, &self.state.canvas_state);
        }
    }

    fn available_layouts(&self) -> Vec<Vec<QukcLayoutRegion>> {
        let filtered_layers: Vec<(&usize, &Layer)> = self
            .state
            .canvas_state
            .layers
            .iter()
            .filter(|layer| layer.1.content.is_photo())
            .collect();

        match filtered_layers.len() {
            1 => {
                vec![
                    vec![QukcLayoutRegion {
                        absolute_rect: Self::fractional_rect_for_layer_in_page(
                            filtered_layers[0].1,
                            &self.state.canvas_state.page.value,
                            Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0)),
                            QuickLayoutFillMode::Fill,
                        ),
                    }],
                    vec![QukcLayoutRegion {
                        absolute_rect: Self::fractional_rect_for_layer_in_page(
                            filtered_layers[0].1,
                            &self.state.canvas_state.page.value,
                            Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0)),
                            QuickLayoutFillMode::Margin(0.1),
                        ),
                    }],
                    vec![QukcLayoutRegion {
                        absolute_rect: Self::fractional_rect_for_layer_in_page(
                            filtered_layers[0].1,
                            &self.state.canvas_state.page.value,
                            Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0)),
                            QuickLayoutFillMode::Margin(0.3),
                        ),
                    }],
                ]
            }
            2 => {
                vec![
                    vec![
                        QukcLayoutRegion {
                            absolute_rect: Self::fractional_rect_for_layer_in_page(
                                filtered_layers[0].1,
                                &self.state.canvas_state.page.value,
                                Rect::from_min_size(Pos2::ZERO, Vec2::new(1.0, 0.5)),
                                QuickLayoutFillMode::Fill,
                            ),
                        },
                        QukcLayoutRegion {
                            absolute_rect: Self::fractional_rect_for_layer_in_page(
                                filtered_layers[1].1,
                                &self.state.canvas_state.page.value,
                                Rect::from_min_size(Pos2::new(0.0, 0.5), Vec2::new(1.0, 0.5)),
                                QuickLayoutFillMode::Fill,
                            ),
                        },
                    ],
                    vec![
                        QukcLayoutRegion {
                            absolute_rect: Self::fractional_rect_for_layer_in_page(
                                filtered_layers[0].1,
                                &self.state.canvas_state.page.value,
                                Rect::from_min_size(Pos2::ZERO, Vec2::new(1.0, 0.5)),
                                QuickLayoutFillMode::Margin(0.2),
                            ),
                        },
                        QukcLayoutRegion {
                            absolute_rect: Self::fractional_rect_for_layer_in_page(
                                filtered_layers[1].1,
                                &self.state.canvas_state.page.value,
                                Rect::from_min_size(Pos2::new(0.0, 0.5), Vec2::new(1.0, 0.5)),
                                QuickLayoutFillMode::Margin(0.2),
                            ),
                        },
                    ],
                    vec![
                        QukcLayoutRegion {
                            absolute_rect: Self::fractional_rect_for_layer_in_page(
                                filtered_layers[0].1,
                                &self.state.canvas_state.page.value,
                                Rect::from_min_size(Pos2::ZERO, Vec2::new(1.0, 0.5)),
                                QuickLayoutFillMode::Margin(0.1),
                            ),
                        },
                        QukcLayoutRegion {
                            absolute_rect: Self::fractional_rect_for_layer_in_page(
                                filtered_layers[1].1,
                                &self.state.canvas_state.page.value,
                                Rect::from_min_size(Pos2::new(0.0, 0.5), Vec2::new(1.0, 0.5)),
                                QuickLayoutFillMode::Margin(0.1),
                            ),
                        },
                    ],
                ]
            }
            _ => vec![],
        }
    }

    fn fractional_rect_for_layer_in_page(
        layer: &Layer,
        page: &Page,
        max_rect_percentage: Rect,
        margin_option: QuickLayoutFillMode,
    ) -> Rect {
        // Convert max_rect_percentage to absolute values based on the page size
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

        // Get the original rect of the layer
        let layer_rect = layer.transform_state.rect;

        // Calculate the aspect ratio of the layer
        let layer_aspect_ratio = layer_rect.width() / layer_rect.height();

        // Calculate the maximum width and height for the rect within the max_rect
        let (max_width, max_height) = match margin_option {
            QuickLayoutFillMode::Fill => (max_rect.width(), max_rect.height()),
            QuickLayoutFillMode::Margin(margin_percentage) => (
                max_rect.width() * (1.0 - margin_percentage),
                max_rect.height() * (1.0 - margin_percentage),
            ),
        };

        // Calculate the new width and height while maintaining the aspect ratio
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

        // Center the new rect within the max_rect
        let x = max_rect.min.x + (max_rect.width() - new_width) / 2.0;
        let y = max_rect.min.y + (max_rect.height() - new_height) / 2.0;

        // Create the new rect
        Rect::from_min_size(
            egui::Pos2::new(x, y),
            egui::Vec2::new(new_width, new_height),
        )
    }
}
