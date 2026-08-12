use eframe::egui::{self, Sense, Slider, Vec2};
use egui_extras::Column;

use crate::{
    layout::{LayoutNode, apply_layout_node, template},
    utils::EguiUiExt,
    widget::canvas::CanvasHistoryManager,
    widget::{
        canvas::{Canvas, CanvasState},
        edit_response::EditResponse,
        spacer::Spacer,
    },
};

#[derive(Debug, PartialEq, Clone)]
pub struct QuickLayoutState {
    gap: f32,
    margin: f32,
}

impl QuickLayoutState {
    pub fn new() -> QuickLayoutState {
        QuickLayoutState {
            gap: 50.0,
            margin: 100.0,
        }
    }
}

#[derive(PartialEq)]
pub struct QuickLayout<'a> {
    pub state: &'a mut QuickLayoutState,
    pub canvas_state: &'a mut CanvasState,
}

impl<'a> QuickLayout<'a> {
    pub fn new(state: &'a mut QuickLayoutState, canvas_state: &'a mut CanvasState) -> Self {
        QuickLayout {
            state,
            canvas_state,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> EditResponse {
        ui.spacing_mut().item_spacing = Vec2::splat(10.0);
        let mut edit_response = EditResponse::none();

        let available_layouts = self.available_layouts();

        if available_layouts.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.heading("Add photos to view available layouts.");
            });

            return edit_response;
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

        let mut selected_layout: Option<LayoutNode> = None;

        ui.vertical(|ui| {
            let mut new_gap = self.state.gap;
            let mut new_margin = self.state.margin;

            ui.horizontal(|ui| {
                ui.label("Gap:");
                let response = ui.add(Slider::new(&mut new_gap, 0.0..=100.0));
                edit_response |= EditResponse::drag(&response);
            });

            ui.horizontal(|ui| {
                ui.label("Margin:");
                let response = ui.add(Slider::new(&mut new_margin, 0.0..=100.0));
                edit_response |= EditResponse::drag(&response);
            });

            if (new_gap != self.state.gap || new_margin != self.state.margin)
                && let Some(last_layout) = self.canvas_state.last_quick_layout.clone()
            {
                apply_layout_node(&last_layout, self.canvas_state, new_gap, new_margin);
            }

            self.state.gap = new_gap;
            self.state.margin = new_margin;

            egui_extras::TableBuilder::new(ui)
                .id_salt("quick_layouts_table")
                .min_scrolled_height(available_height)
                .columns(Column::exact(column_width), num_columns)
                .column(Column::exact(spacer_width))
                .body(|body| {
                    body.rows(row_height, num_rows, |mut row| {
                        let offset = row.index() * num_columns;
                        for i in 0..num_columns {
                            if offset + i >= num_rows {
                                break;
                            }

                            let layout = &available_layouts[offset + i];

                            let mut canvas_state = self.canvas_state.clone();

                            apply_layout_node(
                                layout,
                                &mut canvas_state,
                                self.state.gap,
                                self.state.margin,
                            );

                            row.col(|ui| {
                                ui.push_id(("quick_layout_preview", offset + i), |ui| {
                                    let page_rect = ui.max_rect().shrink2(Vec2::new(20.0, 0.0));
                                    Canvas::new(
                                        &mut canvas_state,
                                        page_rect,
                                        &mut CanvasHistoryManager::preview(),
                                    )
                                    .show_preview(ui, page_rect);

                                    let click_response =
                                        ui.allocate_rect(page_rect, Sense::click());

                                    if click_response.clicked() {
                                        selected_layout = Some(layout.clone());
                                    }
                                });
                            });
                        }

                        row.col(|ui| {
                            ui.add(Spacer::new(spacer_width, row_height));
                        });
                    })
                });
        });

        if let Some(selected_layout) = selected_layout {
            apply_layout_node(
                &selected_layout,
                self.canvas_state,
                self.state.gap,
                self.state.margin,
            );
            self.canvas_state.last_quick_layout = Some(selected_layout);
            edit_response |= EditResponse::discrete(true);
        }
        edit_response
    }

    fn available_layouts(&self) -> Vec<LayoutNode> {
        use std::sync::LazyLock;

        static PRESETS: LazyLock<Vec<template::LayoutPreset>> =
            LazyLock::new(|| template::load_presets(include_str!("../../../layouts/presets.ron")));

        let n = self.canvas_state.quick_layout_items().len();

        if n == 0 {
            return vec![];
        }

        PRESETS
            .iter()
            .filter_map(|preset| preset.resolve(n))
            .collect()
    }
}
