use eframe::egui::{self};
use egui::{Button, Layout, Sense, Stroke, Vec2};

use egui_extras::Column;
use indexmap::IndexMap;

use crate::{
    assets::Asset,
    id::{PageId, next_page_id},
    theme,
    widget::canvas::CanvasHistoryManager,
};

use super::{
    canvas::{Canvas, CanvasState},
    spacer::Spacer,
};

pub enum PagesResponse {
    None,
    SelectPage(PageId),
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
        let column_width = window_width.clamp(1.0, 256.0);
        let row_height = column_width;
        let num_columns = (window_width / column_width).floor().max(1.0) as usize;

        //let padding_size = num_columns as f32 * 10.0;
        let spacer_width = (window_width
            - ((column_width + ui.spacing().item_spacing.x) * num_columns as f32)
            - 10.0
            - ui.spacing().item_spacing.x)
            .max(0.0);

        let num_rows = self.state.pages.len().div_ceil(num_columns);

        let mut clicked_page = None;
        let mut from = None;
        let mut to = None;

        ui.set_clip_rect(ui.available_rect_before_wrap());

        let bottom_bar_height = 50.0;

        let mut table_size = ui.available_size() - Vec2::splat(10.0);
        table_size.y -= bottom_bar_height;

        ui.allocate_ui(table_size, |ui| {
            egui_extras::TableBuilder::new(ui)
                .id_salt("pages_table")
                .min_scrolled_height(table_size.y)
                .drag_to_scroll(egui::scroll_area::DragScroll::Never)
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
                            let page = &mut self.state.pages.get_index(index).unwrap().1.clone();

                            row.col(|ui| {
                                let item_id = ui.make_persistent_id(("page_list", id));

                                ui.vertical(|ui| {
                                    ui.add_space(10.0);

                                    let item_rect = ui.max_rect().shrink(10.0);
                                    let response =
                                        ui.interact(item_rect, item_id, Sense::click_and_drag());
                                    response.dnd_set_drag_payload(id);

                                    ui.horizontal(|ui| {
                                        ui.add_space(10.0);
                                        ui.label(format!("Page {}", index + 1));
                                    });

                                    let mut page_rect = item_rect;
                                    page_rect.min.y += 30.0;
                                    Canvas::new(
                                        page,
                                        page_rect,
                                        &mut CanvasHistoryManager::preview(),
                                    )
                                    .show_preview(ui, page_rect);

                                    if let (Some(pointer), Some(hovered_idx)) = (
                                        ui.input(|i| i.pointer.interact_pos()),
                                        response.dnd_hover_payload::<PageId>(),
                                    ) {
                                        if *hovered_idx != id {
                                            let stroke =
                                                egui::Stroke::new(2.0, theme::color::WHITE);
                                            if pointer.y < page_rect.center().y {
                                                ui.painter().hline(
                                                    page_rect.x_range(),
                                                    page_rect.top(),
                                                    stroke,
                                                );
                                                to = Some(index);
                                            } else {
                                                ui.painter().hline(
                                                    page_rect.x_range(),
                                                    page_rect.bottom(),
                                                    stroke,
                                                );
                                                to = Some(index + 1);
                                            }
                                        }

                                        if let Some(dragged_id) =
                                            response.dnd_release_payload::<PageId>()
                                        {
                                            from = self.state.pages.get_index_of(&*dragged_id);
                                        }
                                    }

                                    if response.clicked() {
                                        clicked_page = Some(id);
                                    }

                                    if self.state.selected_page == id {
                                        // ui.set_clip_rect(ui.max_rect().expand(10.0));
                                        ui.painter().rect_stroke(
                                            item_rect.expand(3.0),
                                            4.0,
                                            Stroke::new(3.0, theme::color::ACCENT),
                                            egui::StrokeKind::Outside,
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

        // Handle reordering
        if let (Some(from_idx), Some(to_idx)) = (from, to)
            && from_idx != to_idx
        {
            let (from_key, from_page) = self.state.pages.get_index(from_idx).unwrap();
            let (from_key, from_page) = (*from_key, from_page.clone());

            self.state.pages.shift_remove(&from_key);

            if to_idx < self.state.pages.len() {
                self.state.pages.shift_insert(to_idx, from_key, from_page);
            } else {
                self.state.pages.insert(from_key, from_page);
            }
        }

        ui.painter()
            .rect_filled(ui.available_rect_before_wrap(), 0.0, theme::color::SURFACE);

        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(20.0);

            if ui
                .add(Button::image_and_text(Asset::add_page(), "Add Page"))
                .on_hover_text("Add a new page")
                .clicked()
            {
                self.state.pages.insert(next_page_id(), CanvasState::new());
            }

            // Only show delete button if we have more than one page
            if self.state.pages.len() > 1
                && ui
                    .add(Button::image_and_text(Asset::add_page(), "Delete Page"))
                    .on_hover_text("Delete current page")
                    .clicked()
                && let Some(index) = self.state.pages.get_index_of(&self.state.selected_page)
            {
                self.state.pages.shift_remove_index(index);
                // Select the previous page, or the first page if we deleted the first one
                clicked_page = Some(
                    *self
                        .state
                        .pages
                        .get_index(index.saturating_sub(1))
                        .unwrap()
                        .0,
                );
            }
        });

        if let Some(page) = clicked_page {
            PagesResponse::SelectPage(page)
        } else {
            PagesResponse::None
        }
    }
}
