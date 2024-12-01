use eframe::egui::{self};
use egui::{Pos2, Rect, Sense, Vec2};

use egui_extras::Column;
use strum::IntoEnumIterator;

use crate::{
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

        let available_layouts = self.available_layouts();

        if available_layouts.is_empty() {
            ui.both_centered(|ui| {
                ui.heading("Add photos to view available layouts.");
            });

            return;
        }

        let available_width = ui.available_width();
        let available_height = ui.available_height();
        let column_width = 256.0;
        let row_height = 256.0;
        let num_columns: usize = (available_width / column_width).floor() as usize;

        //let padding_size = num_columns as f32 * 10.0;
        let spacer_width = (available_width
            - ((column_width + ui.spacing().item_spacing.x) * num_columns as f32)
            - 10.0
            - ui.spacing().item_spacing.x)
            .max(0.0);

        let num_rows = available_layouts.len();

        let mut selected_layout = None;

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
                .save_history(CanvasHistoryKind::QuickLayout, self.state.canvas_state);
        }
    }

    fn available_layouts(&self) -> Vec<Vec<QuickLayoutRegion>> {
        let filtered_layers: Vec<(&usize, &Layer)> = self
            .state
            .canvas_state
            .layers
            .iter()
            .filter(|layer| layer.1.content.is_photo())
            .collect();

        let n = filtered_layers.len();
        if n == 0 {
            return vec![];
        }

        let mut layouts = vec![];

        // Existing layouts for 1 photo
        if n == 1 {
            layouts.push(vec![QuickLayoutRegion {
                absolute_rect: Self::fractional_rect_for_layer_in_page(
                    filtered_layers[0].1,
                    &self.state.canvas_state.page.value,
                    Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0)),
                    QuickLayoutFillMode::Fill,
                ),
            }]);
            layouts.push(vec![QuickLayoutRegion {
                absolute_rect: Self::fractional_rect_for_layer_in_page(
                    filtered_layers[0].1,
                    &self.state.canvas_state.page.value,
                    Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0)),
                    QuickLayoutFillMode::Margin(0.1),
                ),
            }]);
            layouts.push(vec![QuickLayoutRegion {
                absolute_rect: Self::fractional_rect_for_layer_in_page(
                    filtered_layers[0].1,
                    &self.state.canvas_state.page.value,
                    Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0)),
                    QuickLayoutFillMode::Margin(0.3),
                ),
            }]);
            // Full-page photo with padding
            layouts.push(vec![QuickLayoutRegion {
                absolute_rect: Self::fractional_rect_for_layer_in_page(
                    filtered_layers[0].1,
                    &self.state.canvas_state.page.value,
                    Rect::from_min_size(Pos2::new(0.05, 0.05), Vec2::new(0.9, 0.9)),
                    QuickLayoutFillMode::Fill,
                ),
            }]);
        } else if n == 2 {
            layouts.push(vec![
                QuickLayoutRegion {
                    absolute_rect: Self::fractional_rect_for_layer_in_page(
                        filtered_layers[0].1,
                        &self.state.canvas_state.page.value,
                        Rect::from_min_size(Pos2::ZERO, Vec2::new(1.0, 0.5)),
                        QuickLayoutFillMode::Fill,
                    ),
                },
                QuickLayoutRegion {
                    absolute_rect: Self::fractional_rect_for_layer_in_page(
                        filtered_layers[1].1,
                        &self.state.canvas_state.page.value,
                        Rect::from_min_size(Pos2::new(0.0, 0.5), Vec2::new(1.0, 0.5)),
                        QuickLayoutFillMode::Fill,
                    ),
                },
            ]);
            layouts.push(vec![
                QuickLayoutRegion {
                    absolute_rect: Self::fractional_rect_for_layer_in_page(
                        filtered_layers[0].1,
                        &self.state.canvas_state.page.value,
                        Rect::from_min_size(Pos2::ZERO, Vec2::new(1.0, 0.5)),
                        QuickLayoutFillMode::Margin(0.2),
                    ),
                },
                QuickLayoutRegion {
                    absolute_rect: Self::fractional_rect_for_layer_in_page(
                        filtered_layers[1].1,
                        &self.state.canvas_state.page.value,
                        Rect::from_min_size(Pos2::new(0.0, 0.5), Vec2::new(1.0, 0.5)),
                        QuickLayoutFillMode::Margin(0.2),
                    ),
                },
            ]);
            layouts.push(vec![
                QuickLayoutRegion {
                    absolute_rect: Self::fractional_rect_for_layer_in_page(
                        filtered_layers[0].1,
                        &self.state.canvas_state.page.value,
                        Rect::from_min_size(Pos2::ZERO, Vec2::new(1.0, 0.5)),
                        QuickLayoutFillMode::Margin(0.1),
                    ),
                },
                QuickLayoutRegion {
                    absolute_rect: Self::fractional_rect_for_layer_in_page(
                        filtered_layers[1].1,
                        &self.state.canvas_state.page.value,
                        Rect::from_min_size(Pos2::new(0.0, 0.5), Vec2::new(1.0, 0.5)),
                        QuickLayoutFillMode::Margin(0.1),
                    ),
                },
            ]);
            // Side-by-side layout with padding
            layouts.push(vec![
                QuickLayoutRegion {
                    absolute_rect: Self::fractional_rect_for_layer_in_page(
                        filtered_layers[0].1,
                        &self.state.canvas_state.page.value,
                        Rect::from_min_size(Pos2::new(0.05, 0.05), Vec2::new(0.425, 0.9)),
                        QuickLayoutFillMode::Fill,
                    ),
                },
                QuickLayoutRegion {
                    absolute_rect: Self::fractional_rect_for_layer_in_page(
                        filtered_layers[1].1,
                        &self.state.canvas_state.page.value,
                        Rect::from_min_size(Pos2::new(0.525, 0.05), Vec2::new(0.425, 0.9)),
                        QuickLayoutFillMode::Fill,
                    ),
                },
            ]);
        } else if n >= 3 {
            // Grid layout
            layouts.push(self.generate_grid_layout(&filtered_layers, n));

            // Highlight layout
            layouts.push(self.generate_highlight_layout(0.0, &filtered_layers));

            layouts.push(self.generate_highlight_layout(0.1, &filtered_layers));

            // Grid layout with padding
            layouts.push(self.generate_grid_layout_with_padding(&filtered_layers, n));

            // Vertical stack layout with margins
            layouts.push(self.generate_vertical_stack_layout(&filtered_layers));

            // Horizontal stack layout with margins
            layouts.push(self.generate_horizontal_stack_layout(&filtered_layers));

            // Zigzag layout
            layouts.push(self.generate_zigzag_layout(&filtered_layers));
        }

        layouts
    }

    fn generate_grid_layout(
        &self,
        filtered_layers: &[(&usize, &Layer)],
        n: usize,
    ) -> Vec<QuickLayoutRegion> {
        let grid_size = (n as f32).sqrt().ceil() as usize;
        filtered_layers
            .iter()
            .enumerate()
            .map(|(index, (_, layer))| {
                let row = index / grid_size;
                let col = index % grid_size;
                let rect = Rect::from_min_size(
                    Pos2::new(col as f32 / grid_size as f32, row as f32 / grid_size as f32),
                    Vec2::new(1.0 / grid_size as f32, 1.0 / grid_size as f32),
                );
                QuickLayoutRegion {
                    absolute_rect: Self::fractional_rect_for_layer_in_page(
                        layer,
                        &self.state.canvas_state.page.value,
                        rect,
                        QuickLayoutFillMode::Fill,
                    ),
                }
            })
            .collect()
    }

    fn generate_highlight_layout(
        &self,
        padding: f32,
        filtered_layers: &[(&usize, &Layer)],
    ) -> Vec<QuickLayoutRegion> {
        let n = filtered_layers.len();
        let mut regions = vec![];
        // Try to find a portrait photo otherwise use the first photo
        let highlight_layer_index = filtered_layers
            .iter()
            .position(|(_, layer)| {
                layer.transform_state.rect.width() < layer.transform_state.rect.height()
            })
            .unwrap_or_default();

        // Highlighted photo
        let highlight_region = QuickLayoutRegion {
            absolute_rect: Self::fractional_rect_for_layer_in_page(
                filtered_layers[highlight_layer_index].1,
                &self.state.canvas_state.page.value,
                Rect::from_min_size(Pos2::ZERO, Vec2::new(0.6, 1.0)),
                QuickLayoutFillMode::Margin(padding),
            ),
        };

        let highlight_rect = highlight_region.absolute_rect;

        let min_y = highlight_rect.min.y / &self.state.canvas_state.page.value.size_pixels().y;
        let max_y = highlight_rect.max.y / &self.state.canvas_state.page.value.size_pixels().y;

      
        // Remaining photos
        let photo_height = (max_y - min_y) / (n - 1) as f32;
        let mut non_highlight_count = 0;

        for (i, (_, layer)) in filtered_layers.iter().enumerate() {
            if i == highlight_layer_index {
                regions.push(highlight_region.clone());
                continue;
            }
            regions.push(QuickLayoutRegion {
                absolute_rect: Self::fractional_rect_for_layer_in_page(
                    layer,
                    &self.state.canvas_state.page.value,
                    Rect::from_min_size(
                        Pos2::new(0.6, min_y + non_highlight_count as f32 * photo_height),
                        Vec2::new(0.4, photo_height),
                    ),
                    QuickLayoutFillMode::Margin(padding),
                ),
            });

            non_highlight_count += 1;
        }

        regions
    }

    fn generate_grid_layout_with_padding(
        &self,
        filtered_layers: &[(&usize, &Layer)],
        n: usize,
    ) -> Vec<QuickLayoutRegion> {
        let grid_size = (n as f32).sqrt().ceil() as usize;
        let padding = 0.02;
        let cell_size = 1.0 / grid_size as f32;
        filtered_layers
            .iter()
            .enumerate()
            .map(|(index, (_, layer))| {
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
                    absolute_rect: Self::fractional_rect_for_layer_in_page(
                        layer,
                        &self.state.canvas_state.page.value,
                        rect,
                        QuickLayoutFillMode::Fill,
                    ),
                }
            })
            .collect()
    }

    fn generate_vertical_stack_layout(
        &self,
        filtered_layers: &[(&usize, &Layer)],
    ) -> Vec<QuickLayoutRegion> {
        let n = filtered_layers.len();
        let margin = 0.02;
        let available_height = 1.0 - margin * (n as f32 + 1.0);
        let cell_height = available_height / n as f32;
        filtered_layers
            .iter()
            .enumerate()
            .map(|(i, (_, layer))| {
                let y = margin * (i as f32 + 1.0) + cell_height * i as f32;
                QuickLayoutRegion {
                    absolute_rect: Self::fractional_rect_for_layer_in_page(
                        layer,
                        &self.state.canvas_state.page.value,
                        Rect::from_min_size(
                            Pos2::new(margin, y),
                            Vec2::new(1.0 - 2.0 * margin, cell_height),
                        ),
                        QuickLayoutFillMode::Fill,
                    ),
                }
            })
            .collect()
    }

    fn generate_horizontal_stack_layout(
        &self,
        filtered_layers: &[(&usize, &Layer)],
    ) -> Vec<QuickLayoutRegion> {
        let n = filtered_layers.len();
        let margin = 0.02;
        let available_width = 1.0 - margin * (n as f32 + 1.0);
        let cell_width = available_width / n as f32;
        filtered_layers
            .iter()
            .enumerate()
            .map(|(i, (_, layer))| {
                let x = margin * (i as f32 + 1.0) + cell_width * i as f32;
                QuickLayoutRegion {
                    absolute_rect: Self::fractional_rect_for_layer_in_page(
                        layer,
                        &self.state.canvas_state.page.value,
                        Rect::from_min_size(
                            Pos2::new(x, margin),
                            Vec2::new(cell_width, 1.0 - 2.0 * margin),
                        ),
                        QuickLayoutFillMode::Fill,
                    ),
                }
            })
            .collect()
    }

    fn generate_zigzag_layout(
        &self,
        filtered_layers: &[(&usize, &Layer)],
    ) -> Vec<QuickLayoutRegion> {
        let size = 0.3;
        let x_positions = [0.1, 0.6];
        filtered_layers
            .iter()
            .enumerate()
            .map(|(i, (_, layer))| {
                let x = x_positions[i % 2];
                let y = 0.1 + 0.2 * i as f32;
                QuickLayoutRegion {
                    absolute_rect: Self::fractional_rect_for_layer_in_page(
                        layer,
                        &self.state.canvas_state.page.value,
                        Rect::from_min_size(Pos2::new(x, y), Vec2::new(size, size)),
                        QuickLayoutFillMode::Fill,
                    ),
                }
            })
            .collect()
    }

    fn generate_portrait_left_layout(
        &self,
        portrait_layer: &(&usize, &Layer),
        landscape_layers: &[(&usize, &Layer)],
    ) -> Vec<QuickLayoutRegion> {
        let mut regions = vec![];

        // Place the portrait photo on the left, taking full height and 40% width
        regions.push(QuickLayoutRegion {
            absolute_rect: Self::fractional_rect_for_layer_in_page(
                portrait_layer.1,
                &self.state.canvas_state.page.value,
                Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(0.4, 1.0)),
                QuickLayoutFillMode::Fill,
            ),
        });

        // Vertically stack the landscape photos on the right
        let n = landscape_layers.len();
        let photo_height = 1.0 / n as f32;
        for (i, (_, layer)) in landscape_layers.iter().enumerate() {
            regions.push(QuickLayoutRegion {
                absolute_rect: Self::fractional_rect_for_layer_in_page(
                    layer,
                    &self.state.canvas_state.page.value,
                    Rect::from_min_size(
                        Pos2::new(0.4, i as f32 * photo_height),
                        Vec2::new(0.6, photo_height),
                    ),
                    QuickLayoutFillMode::Fill,
                ),
            });
        }

        regions
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
