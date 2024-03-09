use eframe::egui::{self};
use egui::{ComboBox, RichText, Vec2};

use egui_extras::Column;
use strum::IntoEnumIterator;

use crate::{
    utils::EditableValueTextEdit,
    widget::page_canvas::{Page, Unit},
};

use super::{
    page_canvas::{Canvas, CanvasState},
    spacer::Spacer,
};

#[derive(Debug, PartialEq)]
pub struct PagesState<'a> {
    pages: &'a mut Vec<CanvasState>,
}

impl PagesState<'_> {
    pub fn new(pages: &mut Vec<CanvasState>) -> PagesState {
        PagesState { pages }
    }
}

#[derive(Debug, PartialEq)]
pub struct Pages<'a> {
    pub state: &'a mut PagesState<'a>,
}

impl<'a> Pages<'a> {
    pub fn new(state: &'a mut PagesState<'a>) -> Pages<'a> {
        Pages { state }
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

        let num_rows = self.state.pages.len().div_ceil(num_columns);

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

                        row.col(|ui| {
                            ui.label("Page");

                            Canvas::new(&mut self.state.pages[offest + i], ui.max_rect()).show(ui);
                        });
                    }

                    row.col(|ui| {
                        ui.add(Spacer::new(spacer_width, row_height));
                    });
                })
            });
    }
}
