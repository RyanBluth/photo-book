use eframe::egui::Widget;
use egui::{Key, Ui};
use image::metadata;
use std::collections::HashSet;
use strum::IntoEnumIterator;

use crate::dependencies::{Dependency, Singleton, SingletonFor};
use crate::photo::{PhotoMetadataField, PhotoRating, SaveOnDropPhoto};
use crate::photo_manager::PhotoManager;

use super::{
    segment_control::SegmentControl,
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
            egui::Frame::NONE
                .inner_margin(egui::Margin::same(12))
                .fill(egui::Color32::TRANSPARENT)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Rating").small().strong());
                        let mut current_rating = self.photo.rating();
                        ui.add_space(4.0);
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

                        ui.add_space(4.0);

                        ui.label(egui::RichText::new("Tags").small().strong());
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
                            let current_photo_tags = self.photo.tags();

                            for tag in &current_photo_tags {
                                if !self.state.selected_tags.contains(tag) {
                                    self.photo.remove_tag(tag);
                                }
                            }

                            for tag in &self.state.selected_tags {
                                if !current_photo_tags.contains(tag) {
                                    self.photo.add_tag(tag.clone());
                                }
                            }
                            self.state.last_photo_tags = self.photo.tags();
                        }

                        ui.add_space(4.0);

                        for (label, value) in self.photo.metadata.iter() {
                            ui.label(egui::RichText::new(format!("{}", label)).small().strong());
                            ui.horizontal_wrapped(|ui| {
                                ui.label(format!("{}", value));
                                if let PhotoMetadataField::Path(path) = value {
                                    if ui.button("📂").clicked() {
                                        open::that_in_background(path.parent().unwrap());
                                    }
                                }
                            });

                            ui.add_space(4.0);
                        }
                    });
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
