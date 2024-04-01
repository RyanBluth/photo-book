use eframe::egui::{self};
use egui::{Sense, Vec2};

use egui_extras::Column;

use crate::{scene::canvas_scene::CanvasHistoryManager, utils::EguiExt};

use super::{
    page_canvas::{Canvas, CanvasState},
    spacer::Spacer,
};

pub enum PagesResponse {
    None,
    SelectPage(usize),
}

#[derive(Debug, PartialEq)]
pub struct PagesState {
    // This should probably be an indexmap where each page has an id
    pub pages: Vec<CanvasState>,
}

impl PagesState {
    pub fn new(pages: Vec<CanvasState>) -> PagesState {
        PagesState { pages }
    }

    pub fn update_pages(&mut self, pages: Vec<CanvasState>) {
        self.pages = pages;
    }
}

#[derive(Debug, PartialEq)]
pub struct Pages<'a> {
    pub state: &'a mut PagesState,
}

impl<'a> Pages<'a> {
    pub fn new(state: &'a mut PagesState) -> Pages<'a> {
        Pages { state }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> PagesResponse {
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

        let num_rows = self.state.pages.len().div_ceil(num_columns);

        if ui
            .button("Add Page")
            .on_hover_text("Add a new page")
            .clicked()
        {
            self.state.pages.push(CanvasState::new());
        }

        let mut clicked_page = None;

        egui_extras::TableBuilder::new(ui)
            .min_scrolled_height(window_height)
            .columns(Column::exact(column_width), num_columns)
            .column(Column::exact(spacer_width))
            .body(|body| {
                body.rows(row_height, num_rows, |mut row| {
                    let offest = row.index() * num_columns;
                    for i in 0..num_columns {
                        if offest + i >= self.state.pages.len() {
                            break;
                        }

                        if row
                            .col(|ui| {
                                Canvas::new(
                                    &mut self.state.pages[offest + i],
                                    ui.max_rect(),
                                    &mut CanvasHistoryManager::new(),
                                )
                                .show_preview(ui, ui.max_rect());
                            })
                            .1
                            .interact(Sense::click())
                            .clicked()
                            {
                            clicked_page = Some(offest + i);
                        }
                    }

                    row.col(|ui| {
                        ui.add(Spacer::new(spacer_width, row_height));
                    });
                })
            });

        if let Some(page) = clicked_page {
            PagesResponse::SelectPage(page)
        } else {
            PagesResponse::None
        }
    }
}
