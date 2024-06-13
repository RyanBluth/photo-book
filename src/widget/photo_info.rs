use eframe::egui::{Grid, Widget};

use crate::photo::{Photo, PhotoMetadataField};

use super::spacer::Spacer;

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
                .num_columns(4)
                .show(ui, |ui| {
                    for (label, value) in self.photo.metadata.iter() {
                        ui.label(format!("{}", label));
                        ui.label(format!("{}", value));
                        if let PhotoMetadataField::Path(path) = value {
                            if ui.button("📂").clicked() {
                                open::that_in_background(path.parent().unwrap());
                            }
                        }
                        Spacer::new(ui.available_width(), 1.0).ui(ui);
                        ui.end_row();
                    }
                });
        })
        .response
    }
}
