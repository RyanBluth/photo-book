use eframe::egui::{Grid, Widget};
use egui::{ComboBox, Ui};
use strum::IntoEnumIterator;

use crate::photo::{Photo, PhotoMetadataField, PhotoRating, SaveOnDropPhoto};

use super::spacer::Spacer;

pub struct PhotoInfo<'a> {
    pub photo: SaveOnDropPhoto<'a>,
}

impl<'a> PhotoInfo<'a> {
    pub fn new(photo: SaveOnDropPhoto<'a>) -> Self {
        Self { photo }
    }
}

impl<'a> PhotoInfo<'a> {
    pub fn show(&mut self, ui: &mut Ui) {
        ui.allocate_ui(ui.available_size(), |ui: &mut egui::Ui| {
            Grid::new("photo_info_grid")
                .striped(true)
                .num_columns(4)
                .show(ui, |ui| {
                    ui.label("Rating");

                    ComboBox::from_id_source("rating_combo_box")
                        .selected_text(format!("{:?}", self.photo.rating))
                        .show_ui(ui, |ui| {
                            for rating in PhotoRating::iter() {
                                ui.selectable_value(
                                    &mut self.photo.rating,
                                    rating,
                                    format!("{:?}", rating),
                                );
                            }
                        });
                    ui.end_row();

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
        });
    }
}
