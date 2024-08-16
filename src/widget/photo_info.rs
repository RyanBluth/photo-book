use eframe::egui::{Grid, Widget};
use egui::{Key, Ui};
use strum::IntoEnumIterator;

use crate::photo::{PhotoMetadataField, PhotoRating, SaveOnDropPhoto};

use super::{segment_control::SegmentControl, spacer::Spacer};

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

                    SegmentControl::new(
                        PhotoRating::iter()
                            .enumerate()
                            .map(|pr| (pr.1, format!("({}) {}", pr.0 + 1, pr.1)))
                            .collect::<Vec<_>>()
                            .as_slice(),
                        &mut self.photo.rating,
                    )
                    .ui(ui);

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

        ui.ctx().input(|input| {
            if input.key_down(Key::Num1) {
                self.photo.rating = PhotoRating::Yes;
            } else if input.key_down(Key::Num2) {
                self.photo.rating = PhotoRating::Maybe;
            } else if input.key_down(Key::Num3) {
                self.photo.rating = PhotoRating::No;
            }
        })
    }
}
