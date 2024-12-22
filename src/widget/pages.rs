use eframe::egui::{self};
use egui::{Button, Color32, Layout, Sense, Stroke, Vec2};

use egui_extras::Column;
use indexmap::IndexMap;

use crate::{
    assets::Asset,
    id::{next_page_id, PageId},
    scene::canvas_scene::{CanvasHistory, CanvasHistoryManager},
    theme,
};

use super::{
    page_canvas::{Canvas, CanvasState},
    spacer::Spacer,
};

pub enum PagesResponse {
    None,
    SelectPage,
}

#[derive(Debug, PartialEq, Clone)]
pub struct PagesState {
    // This should probably be an indexmap where each page has an id
    pub pages: IndexMap<PageId, CanvasState>,

    pub selected_page: PageId,
}

impl PagesState {
    pub fn new(pages: IndexMap<usize, CanvasState>, selected_page: PageId) -> PagesState {
        PagesState {
            pages,
            selected_page,
        }
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
        let column_width = 256.0;
        let row_height = 256.0;
        let num_columns: usize = (window_width / column_width).floor() as usize;

        //let padding_size = num_columns as f32 * 10.0;
        let spacer_width = (window_width
            - ((column_width + ui.spacing().item_spacing.x) * num_columns as f32)
            - 10.0
            - ui.spacing().item_spacing.x)
            .max(0.0);

        let num_rows = self.state.pages.len().div_ceil(num_columns.max(1));

        let mut clicked_page = None;

        ui.set_clip_rect(ui.available_rect_before_wrap());

        let bottom_bar_height = 50.0;

        let mut table_size = ui.available_size();
        table_size.y -= bottom_bar_height;

        ui.allocate_ui(table_size, |ui| {
            egui_extras::TableBuilder::new(ui)
                .min_scrolled_height(table_size.y)
                .auto_shrink(false)
                .columns(Column::exact(column_width), num_columns)
                .column(Column::exact(spacer_width))
                .body(|body| {
                    body.rows(row_height, num_rows, |mut row| {
                        let offset = row.index() * num_columns;
                        for i in 0..num_columns {
                            if offset + i >= self.state.pages.len() {
                                break;
                            }

                            let index: usize = offset + i;
                            let id: usize = *self.state.pages.get_index(index).unwrap().0;
                            let page = self.state.pages.get_index_mut(index).unwrap().1;

                            row.col(|ui| {
                                ui.vertical(|ui| {
                                    ui.add_space(10.0);

                                    ui.label(format!("Page {}", index + 1));

                                    let mut page_rect = ui.max_rect();
                                    page_rect.min.y += 30.0;

                                    Canvas::new(
                                        page,
                                        page_rect,
                                        &mut CanvasHistoryManager::preview(),
                                    )
                                    .show_preview(ui, page_rect);

                                    let click_response =
                                        ui.allocate_rect(page_rect, Sense::click());

                                    if click_response.clicked() {
                                        clicked_page = Some(id);
                                    }

                                    if self.state.selected_page == id {
                                        // Expand the clip rect for the highlight
                                        ui.set_clip_rect(ui.max_rect().expand(10.0));

                                        ui.painter().rect_stroke(
                                            page_rect,
                                            4.0,
                                            Stroke::new(3.0, theme::color::FOCUSED),
                                        );
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

        ui.painter()
            .rect_filled(ui.available_rect_before_wrap(), 0.0, Color32::from_gray(40));

        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(20.0);

            if ui
                .add(Button::image_and_text(Asset::add_page(), "Add Page"))
                .on_hover_text("Add a new page")
                .clicked()
            {
                self.state.pages.insert(next_page_id(), CanvasState::new());
            }
        });

        if let Some(page) = clicked_page {
            self.state.selected_page = page;
            PagesResponse::SelectPage
        } else {
            PagesResponse::None
        }
    }
}
