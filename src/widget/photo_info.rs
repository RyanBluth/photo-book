use std::fmt::format;

use eframe::egui::{Grid, Widget};
use egui_extras::{Column, TableBody, TableBuilder};

use crate::photo::Photo;

pub struct PhotoInfo<'a> {
    pub photo: &'a Photo,
}

impl<'a> PhotoInfo<'a> {
    pub fn new(photo: &'a Photo) -> Self {
        Self { photo }
    }
}

impl<'a> Widget for PhotoInfo<'a> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> eframe::egui::Response {
        ui.allocate_ui(ui.available_size(), |ui| {
            Grid::new("photo_info_grid")
                .striped(true)
                .num_columns(2)
                .max_col_width(200.0)
                .show(ui, |ui| {
                    for (label, value) in self.photo.metadata.fields.iter() {
                        ui.label(format!("{}", label));
                        ui.label(format!("{}", value));
                        ui.wrap_text();
                        ui.end_row()
                    }
                });
        })
        .response
    }
}
