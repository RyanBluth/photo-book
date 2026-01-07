use eframe::egui::{Grid, Widget};
use egui::{Key, Ui};
use std::collections::HashSet;
use strum::IntoEnumIterator;

use crate::dependencies::{Dependency, Singleton, SingletonFor};
use crate::photo::{PhotoMetadataField, PhotoRating, SaveOnDropPhoto};
use crate::photo_manager::PhotoManager;

use super::{
    segment_control::SegmentControl,
    spacer::Spacer,
    tag_chips::{TagChips, TagChipsState},
};

#[derive(Debug, Clone)]
pub struct PhotoInfoState {
    pub tag_chips_state: TagChipsState,
    pub selected_tags: HashSet<String>,
    pub last_photo_tags: HashSet<String>,
}

impl PhotoInfoState {
    pub fn new() -> Self {
        Self {
            tag_chips_state: TagChipsState::new(),
            selected_tags: HashSet::new(),
            last_photo_tags: HashSet::new(),
        }
    }
}

pub struct PhotoInfo<'a> {
    pub photo: SaveOnDropPhoto<'a>,
    pub state: &'a mut PhotoInfoState,
}

impl<'a> PhotoInfo<'a> {
    pub fn new(photo: SaveOnDropPhoto<'a>, state: &'a mut PhotoInfoState) -> Self {
        Self { photo, state }
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

                    let mut current_rating = self.photo.rating();
                    SegmentControl::new(
                        PhotoRating::iter()
                            .enumerate()
                            .map(|pr| (pr.1, format!("({}) {}", pr.0 + 1, pr.1)))
                            .collect::<Vec<_>>()
                            .as_slice(),
                        &mut current_rating,
                    )
                    .ui(ui);

                    if current_rating != self.photo.rating() {
                        self.photo.set_rating(current_rating);
                    }

                    ui.end_row();

                    // Tags section
                    ui.label("Tags");
                    ui.vertical(|ui| {
                        // Sync selected_tags with photo tags only if photo has changed
                        let photo_tags = self.photo.tags();
                        if self.state.last_photo_tags != photo_tags {
                            self.state.selected_tags = photo_tags.clone();
                            self.state.last_photo_tags = photo_tags.clone();
                        }

                        let photo_manager: Singleton<PhotoManager> = Dependency::get();
                        let available_tags = photo_manager.with_lock(|pm| pm.all_tags());

                        let tag_response = TagChips::new(
                            &mut self.state.selected_tags,
                            &mut self.state.tag_chips_state,
                        )
                        .available_tags(&available_tags)
                        .show_input(true)
                        .show(ui);

                        if tag_response.changed() {
                            // Sync changes to photo
                            let current_photo_tags = self.photo.tags();

                            // Remove tags that are no longer selected
                            for tag in &current_photo_tags {
                                if !self.state.selected_tags.contains(tag) {
                                    self.photo.remove_tag(tag);
                                }
                            }

                            // Add new tags that are selected but not in photo
                            for tag in &self.state.selected_tags {
                                if !current_photo_tags.contains(tag) {
                                    self.photo.add_tag(tag.clone());
                                }
                            }
                            // Update our tracking of photo tags
                            self.state.last_photo_tags = self.photo.tags();
                        }
                    });
                    Spacer::new(ui.available_width(), 1.0).ui(ui);
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
                self.photo.set_rating(PhotoRating::Yes);
            } else if input.key_down(Key::Num2) {
                self.photo.set_rating(PhotoRating::Maybe);
            } else if input.key_down(Key::Num3) {
                self.photo.set_rating(PhotoRating::No);
            }
        })
    }
}
