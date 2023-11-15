use std::fmt::format;

use eframe::egui::{Grid, Widget, ImageButton};
use egui_extras::{Column, TableBody, TableBuilder};

use crate::photo::{Photo, PhotoMetadataField};

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
                .num_columns(3)
                .max_col_width(200.0)
                .show(ui, |ui| {
                    for (label, value) in self.photo.metadata.iter() {
                        ui.label(format!("{}", label));
                        ui.label(format!("{}", value));
                        if let PhotoMetadataField::Path(path) = value {
                            if ui.button("📂").clicked() {
                                open::that_in_background(&path.parent().unwrap());
                                
                            }
                        }
                        ui.end_row()
                    }
                });
        })
        .response
    }
}
