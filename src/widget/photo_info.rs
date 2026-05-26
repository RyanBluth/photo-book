use egui::{Key, Ui};
use std::collections::HashSet;

use crate::cursor_manager::CursorManager;
use crate::model::album::AlbumId;
use crate::photo::{PhotoMetadataField, SaveOnDropPhoto};
use crate::photo_manager::PhotoManager;
use crate::theme::color;
use crate::{dep, dep_mut};

use super::tag_chips::{TagChips, TagChipsState};

#[derive(Debug, Clone)]
pub struct PhotoInfoState {
    pub tag_chips_state: TagChipsState,
    pub selected_tags: HashSet<String>,
    pub last_photo_tags: HashSet<String>,
    pub selected_albums: HashSet<AlbumId>,
    pub last_photo_albums: HashSet<AlbumId>,
}

impl PhotoInfoState {
    pub fn new() -> Self {
        Self {
            tag_chips_state: TagChipsState::new(),
            selected_tags: HashSet::new(),
            last_photo_tags: HashSet::new(),
            selected_albums: HashSet::new(),
            last_photo_albums: HashSet::new(),
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
                .fill(color::TRANSPARENT)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Rating").small().strong());
                        let current_rating = self.photo.rating();
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let star_size = 24.0;

                            for i in 1..=3 {
                                let is_selected = current_rating.map_or(false, |r| r >= i as u8);
                                let text = if is_selected { "★" } else { "☆" };
                                let resp = ui.add(
                                    egui::Button::new(
                                        egui::RichText::new(text).size(star_size).color(
                                            if is_selected {
                                                ui.style().visuals.text_color()
                                            } else {
                                                ui.style().visuals.weak_text_color()
                                            },
                                        ),
                                    )
                                    .frame(false),
                                );

                                if resp.hovered() {
                                    dep_mut!(CursorManager, |cm| {
                                        cm.set_cursor(egui::CursorIcon::PointingHand);
                                    });
                                }

                                if resp.clicked() {
                                    if current_rating == Some(i as u8) {
                                        self.photo.set_rating(None);
                                    } else {
                                        self.photo.set_rating(Some(i as u8));
                                    }
                                }
                            }
                        });

                        ui.add_space(4.0);

                        ui.label(egui::RichText::new("Tags").small().strong());
                        let photo_tags = self.photo.tags();
                        if self.state.last_photo_tags != photo_tags {
                            self.state.selected_tags = photo_tags.clone();
                            self.state.last_photo_tags = photo_tags.clone();
                        }

                        let available_tags = dep!(PhotoManager, |pm| pm.all_tags());

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

                        ui.label(egui::RichText::new("Albums").small().strong());
                        self.show_albums(ui);

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
            if input.key_pressed(Key::Num1) {
                self.photo.set_rating(Some(1));
            } else if input.key_pressed(Key::Num2) {
                self.photo.set_rating(Some(2));
            } else if input.key_pressed(Key::Num3) {
                self.photo.set_rating(Some(3));
            } else if input.key_pressed(Key::Num0) {
                self.photo.set_rating(None);
            }
        })
    }

    fn show_albums(&mut self, ui: &mut Ui) {
        let photo_path = self.photo.path.clone();
        let (mut available_albums, photo_albums) = dep!(PhotoManager, |pm| {
            (
                pm.albums_iter()
                    .map(|album| (album.id.clone(), album.name.clone()))
                    .collect::<Vec<_>>(),
                pm.get_photo_albums(&photo_path),
            )
        });

        available_albums
            .sort_by(|(_, left), (_, right)| left.to_lowercase().cmp(&right.to_lowercase()));

        if self.state.last_photo_albums != photo_albums {
            self.state.selected_albums = photo_albums.clone();
            self.state.last_photo_albums = photo_albums;
        }

        if available_albums.is_empty() {
            ui.label(egui::RichText::new("No albums").weak());
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("photo_info_albums")
            .max_height(120.0)
            .show(ui, |ui| {
                for (album_id, album_name) in available_albums {
                    let mut is_selected = self.state.selected_albums.contains(&album_id);
                    if ui.checkbox(&mut is_selected, &album_name).changed() {
                        if is_selected {
                            dep_mut!(PhotoManager, |pm| {
                                pm.add_to_album(&album_id, &photo_path);
                            });
                            self.state.selected_albums.insert(album_id.clone());
                        } else {
                            dep_mut!(PhotoManager, |pm| {
                                pm.remove_from_album(&album_id, &photo_path);
                            });
                            self.state.selected_albums.remove(&album_id);
                        }

                        self.state.last_photo_albums = self.state.selected_albums.clone();
                    }
                }
            });
    }
}
