use chrono::{DateTime, Local, Utc};
use egui::{Key, Label, Margin, RichText, Sense, Ui, Vec2};
use std::collections::HashSet;

use crate::cursor_manager::CursorManager;
use crate::histogram_manager::{HistogramLoadResult, HistogramManager};
use crate::model::album::AlbumId;
use crate::model::editable_value::EditableValue;
use crate::photo::{PhotoMetadataField, PhotoMetadataFieldLabel, Rational, SaveOnDropPhoto};
use crate::photo_manager::PhotoManager;
use crate::theme::color;
use crate::{dep, dep_mut};

use super::{
    autocomplete::{Autocomplete, AutocompleteState},
    chip_collection::chip_collection,
    histogram::Histogram,
    photo_adjustments::{PhotoAdjustmentsEditor, PhotoAdjustmentsState},
    tag_chips::TagChipsState,
};

const PHOTO_INFO_ITEM_SPACING: f32 = 14.0;

#[derive(Debug)]
pub struct PhotoInfoState {
    pub tag_chips_state: TagChipsState,
    pub tag_autocomplete_state: AutocompleteState,
    pub selected_tags: HashSet<String>,
    pub last_photo_tags: HashSet<String>,
    pub selected_albums: HashSet<AlbumId>,
    pub last_photo_albums: HashSet<AlbumId>,
    pub adjustments_state: PhotoAdjustmentsState,
}

impl PhotoInfoState {
    pub fn new() -> Self {
        Self {
            tag_chips_state: TagChipsState::new(),
            tag_autocomplete_state: AutocompleteState::new(),
            selected_tags: HashSet::new(),
            last_photo_tags: HashSet::new(),
            selected_albums: HashSet::new(),
            last_photo_albums: HashSet::new(),
            adjustments_state: PhotoAdjustmentsState::new(),
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
                .inner_margin(Margin::same(8))
                .fill(color::SIDE_PANEL_BACKGROUND)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("photo_info_scroll")
                        .auto_shrink([false, false])
                        .content_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = PHOTO_INFO_ITEM_SPACING;

                                self.show_header(ui);
                                self.show_metadata_card(ui);
                                Self::section_separator(ui);
                                self.show_rating(ui);
                                Self::section_separator(ui);
                                self.show_tags(ui);
                                Self::section_separator(ui);
                                self.show_albums_section(ui);
                                Self::section_separator(ui);
                                self.show_adjustments(ui);
                            });
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

    fn section_separator(ui: &mut Ui) {
        ui.separator();
    }

    fn show_header(&self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.add(
                Label::new(RichText::new(self.photo.file_name()).size(15.0).strong()).truncate(),
            );

            if let Some(date) = self.photo_date() {
                ui.label(RichText::new(Self::format_date(date)).size(15.0).strong());
            }

            let path = self.photo.path.to_string_lossy();
            let path_response = ui.add(
                Label::new(
                    RichText::new(path.as_ref())
                        .size(12.0)
                        .color(color::SURFACE_EMPHASIS),
                )
                .wrap()
                .sense(Sense::click()),
            );

            if path_response.hovered() {
                dep_mut!(CursorManager, |cursor_manager| {
                    cursor_manager.set_cursor(egui::CursorIcon::PointingHand);
                });
            }

            if path_response.clicked()
                && let Some(parent) = self.photo.path.parent()
            {
                open::that_in_background(parent);
            }
        });
    }

    fn show_metadata_card(&self, ui: &mut Ui) {
        egui::Frame::NONE
            .inner_margin(egui::Margin::same(10))
            .corner_radius(8)
            .fill(color::SURFACE)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.set_min_width(ui.available_width());

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_max_width(ui.available_width());
                            ui.add(
                                Label::new(RichText::new(self.camera()).size(15.0).strong())
                                    .truncate(),
                            );
                            ui.add_space(6.0);
                            ui.label(RichText::new("No lens information").size(14.0).strong());
                        });
                    });

                    ui.add_space(8.0);
                    self.show_file_summary_row(ui);

                    Self::separator(ui);

                    self.show_exif_summary_row(ui);
                });
            });
    }

    fn show_rating(&mut self, ui: &mut Ui) {
        let current_rating = self.photo.rating();
        ui.vertical(|ui| {
            ui.label(RichText::new("Rating").small().strong());
            ui.horizontal(|ui| {
                let star_size = 21.0;

                for i in 1..=3 {
                    let is_selected = current_rating.is_some_and(|rating| rating >= i as u8);
                    let text = if is_selected { "★" } else { "☆" };
                    let response = ui.add(
                        egui::Button::new(RichText::new(text).size(star_size).color(
                            if is_selected {
                                ui.style().visuals.text_color()
                            } else {
                                ui.style().visuals.weak_text_color()
                            },
                        ))
                        .frame(false),
                    );

                    if response.hovered() {
                        dep_mut!(CursorManager, |cursor_manager| {
                            cursor_manager.set_cursor(egui::CursorIcon::PointingHand);
                        });
                    }

                    if response.clicked() {
                        if current_rating == Some(i as u8) {
                            self.photo.set_rating(None);
                        } else {
                            self.photo.set_rating(Some(i as u8));
                        }
                    }
                }

                if current_rating.is_some() {
                    ui.add_space(8.0);
                    let clear_response =
                        ui.add(egui::Button::new(RichText::new("Clear").size(12.0)).frame(false));

                    if clear_response.hovered() {
                        dep_mut!(CursorManager, |cursor_manager| {
                            cursor_manager.set_cursor(egui::CursorIcon::PointingHand);
                        });
                    }

                    if clear_response.clicked() {
                        self.photo.set_rating(None);
                    }
                }
            });
        });
    }

    fn show_histogram(
        &mut self,
        ui: &mut Ui,
        adjustments: &crate::model::photo_adjustments::PhotoAdjustments,
    ) {
        let refresh_adjusted = !ui.input(|input| input.pointer.primary_down());
        let histogram = dep_mut!(HistogramManager, |manager| manager.get(
            &self.photo.path,
            adjustments,
            refresh_adjusted,
        ));

        match &histogram {
            HistogramLoadResult::Ready(data) => {
                ui.add(Histogram::new(data).height(82.0));
            }
            HistogramLoadResult::Pending(Some(data)) => {
                ui.add(Histogram::new(data).height(82.0).loading(true));
            }
            HistogramLoadResult::Unavailable(error, Some(data)) => {
                let _ = error.as_str();
                ui.add(Histogram::new(data).height(82.0));
            }
            HistogramLoadResult::Pending(None) => {
                ui.add(Histogram::unavailable().height(82.0).loading(true));
            }
            HistogramLoadResult::Unavailable(error, None) => {
                let _ = error.as_str();
                ui.add(Histogram::unavailable().height(82.0));
            }
        }
    }

    fn show_adjustments(&mut self, ui: &mut Ui) {
        let mut adjustments = self.photo.adjustments();
        self.show_histogram(ui, &adjustments);
        ui.add_space(10.0);
        let original_adjustments = crate::model::photo_adjustments::PhotoAdjustments::default();
        let histogram = dep_mut!(HistogramManager, |manager| {
            manager
                .get(&self.photo.path, &original_adjustments, true)
                .ready_data()
                .cloned()
        });
        if PhotoAdjustmentsEditor::new(&mut adjustments, &mut self.state.adjustments_state)
            .histogram(histogram.as_ref())
            .show(ui)
        {
            self.photo.set_adjustments(adjustments);
        }
    }

    fn show_tags(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Tags").small().strong());
        ui.add_space(4.0);

        let photo_tags = self.photo.tags();
        if self.state.last_photo_tags != photo_tags {
            self.state.selected_tags = photo_tags.clone();
            self.state.last_photo_tags = photo_tags.clone();
        }

        let mut changed = false;

        if self.state.selected_tags.is_empty() {
            ui.label(RichText::new("No tags").weak());
        } else {
            let mut selected_tags: Vec<String> = self.state.selected_tags.iter().cloned().collect();
            selected_tags.sort();

            let chip_response = chip_collection(ui, &selected_tags, None, true, 6.0);
            if let Some(closed_idx) = chip_response.closed_item()
                && let Some(tag) = selected_tags.get(closed_idx)
            {
                self.state.selected_tags.remove(tag);
                changed = true;
            }
        }

        let available_tags = dep!(PhotoManager, |photo_manager| photo_manager.all_tags());
        let available_tags: Vec<String> = available_tags
            .into_iter()
            .filter(|tag| !self.state.selected_tags.contains(tag))
            .collect();

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let add_button_width = 42.0;
            let gap = 8.0;
            let autocomplete_width = (ui.available_width() - add_button_width - gap).max(80.0);
            let autocomplete_response = Autocomplete::new(
                "photo_info_tag_autocomplete",
                self.state.tag_chips_state.tag_input.editable_value(),
                &available_tags,
                &mut self.state.tag_autocomplete_state,
            )
            .hint_text("Add tag")
            .desired_width(autocomplete_width)
            .show(ui);

            if autocomplete_response.response.gained_focus() {
                self.state.tag_chips_state.tag_input.begin_editing();
            }

            if let Some(tag) = autocomplete_response
                .selected_text()
                .or_else(|| autocomplete_response.submitted_text())
            {
                changed |= self.add_tag(tag.to_string());
            }

            ui.add_space(gap);
            let add_response = ui.add_sized(
                [add_button_width, ui.spacing().interact_size.y],
                egui::Button::new(RichText::new("Add").size(13.0)),
            );

            if add_response.hovered() {
                dep_mut!(CursorManager, |cursor_manager| {
                    cursor_manager.set_cursor(egui::CursorIcon::PointingHand);
                });
            }

            if add_response.clicked() {
                changed |= self.add_tag_from_input();
            }
        });

        if changed {
            self.commit_tags();
        }
    }

    fn add_tag_from_input(&mut self) -> bool {
        self.state.tag_chips_state.tag_input.end_editing();
        let tag = self
            .state
            .tag_chips_state
            .tag_input
            .value()
            .trim()
            .to_string();

        self.add_tag(tag)
    }

    fn add_tag(&mut self, tag: String) -> bool {
        let tag = tag.trim().to_string();

        if tag.is_empty() || self.state.selected_tags.contains(&tag) {
            return false;
        }

        self.state.selected_tags.insert(tag);
        self.state.tag_chips_state.tag_input = EditableValue::new(String::new());
        self.state.tag_autocomplete_state.close();
        true
    }

    fn commit_tags(&mut self) {
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

    fn show_albums_section(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Albums").small().strong());
        ui.add_space(4.0);
        self.show_albums(ui);
    }

    fn separator(ui: &mut Ui) {
        ui.add_space(7.0);
        ui.separator();
        ui.add_space(7.0);
    }

    fn show_file_summary_row(&self, ui: &mut Ui) {
        let file_size = self.file_size().unwrap_or_else(|| "-".to_string());

        ui.columns(3, |columns| {
            Self::metadata_cell(&mut columns[0], self.dimensions(), 15.0, egui::Align::LEFT);
            Self::metadata_cell(&mut columns[1], file_size, 15.0, egui::Align::Center);
            Self::metadata_cell(
                &mut columns[2],
                self.file_type_badge(),
                11.0,
                egui::Align::RIGHT,
            );
        });
    }

    fn show_exif_summary_row(&self, ui: &mut Ui) {
        let values = [
            self.iso(),
            self.focal_length(),
            "0 ev".to_string(),
            self.aperture(),
            self.shutter_speed(),
        ];

        let column_count = if ui.available_width() < 290.0 { 3 } else { 5 };
        for chunk in values.chunks(column_count) {
            ui.columns(column_count, |columns| {
                for (column, value) in columns.iter_mut().zip(chunk) {
                    Self::metadata_cell(column, value, 13.0, egui::Align::LEFT);
                }
            });
        }
    }

    fn metadata_cell(ui: &mut Ui, text: impl Into<String>, size: f32, align: egui::Align) {
        let text = text.into();
        let cell_size = Vec2::new(ui.available_width(), ui.spacing().interact_size.y);
        let layout = match align {
            egui::Align::Center => egui::Layout::left_to_right(egui::Align::Center)
                .with_main_justify(true)
                .with_cross_justify(true),
            egui::Align::RIGHT => egui::Layout::right_to_left(egui::Align::Center),
            _ => egui::Layout::left_to_right(egui::Align::Center),
        };

        ui.allocate_ui_with_layout(cell_size, layout, |ui| {
            ui.add(
                Label::new(RichText::new(text).size(size).strong())
                    .truncate()
                    .selectable(false),
            );
        });
    }

    fn field(&self, label: PhotoMetadataFieldLabel) -> Option<&PhotoMetadataField> {
        self.photo.metadata.get(label)
    }

    fn camera(&self) -> String {
        match self.field(PhotoMetadataFieldLabel::Camera) {
            Some(PhotoMetadataField::Camera(camera)) if !camera.trim().is_empty() => {
                camera.trim().to_string()
            }
            _ => "No camera information".to_string(),
        }
    }

    fn dimensions(&self) -> String {
        let width = self.photo.metadata.rotated_width();
        let height = self.photo.metadata.rotated_height();

        if width > 0 && height > 0 {
            format!("{} x {}", width, height)
        } else {
            "Unknown size".to_string()
        }
    }

    fn file_type_badge(&self) -> String {
        self.photo
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_uppercase())
            .unwrap_or_else(|| "FILE".to_string())
    }

    fn file_size(&self) -> Option<String> {
        let bytes = self.photo.path.metadata().ok()?.len() as f64;
        let units = ["B", "KB", "MB", "GB"];
        let mut value = bytes;
        let mut unit_idx = 0;

        while value >= 1024.0 && unit_idx < units.len() - 1 {
            value /= 1024.0;
            unit_idx += 1;
        }

        if unit_idx == 0 {
            Some(format!("{} {}", value as u64, units[unit_idx]))
        } else {
            Some(format!("{:.1} {}", value, units[unit_idx]))
        }
    }

    fn photo_date(&self) -> Option<DateTime<Utc>> {
        match self.field(PhotoMetadataFieldLabel::DateTime) {
            Some(PhotoMetadataField::DateTime(date_time)) => Some(*date_time),
            _ => self.photo.last_modified,
        }
    }

    fn format_date(date_time: DateTime<Utc>) -> String {
        date_time
            .with_timezone(&Local)
            .format("%B %-d, %Y   %-I:%M:%S %p")
            .to_string()
    }

    fn iso(&self) -> String {
        match self.field(PhotoMetadataFieldLabel::ISO) {
            Some(PhotoMetadataField::ISO(iso)) => format!("ISO {}", iso),
            _ => "ISO -".to_string(),
        }
    }

    fn focal_length(&self) -> String {
        match self.field(PhotoMetadataFieldLabel::FocalLength) {
            Some(PhotoMetadataField::FocalLength(focal_length)) => {
                format!("{} mm", Self::format_rational_value(focal_length))
            }
            _ => "- mm".to_string(),
        }
    }

    fn aperture(&self) -> String {
        match self.field(PhotoMetadataFieldLabel::Aperture) {
            Some(PhotoMetadataField::Aperture(aperture)) => {
                format!("f/{}", Self::format_rational_value(aperture))
            }
            _ => "f/-".to_string(),
        }
    }

    fn shutter_speed(&self) -> String {
        match self.field(PhotoMetadataFieldLabel::ShutterSpeed) {
            Some(PhotoMetadataField::ShutterSpeed(shutter_speed)) => {
                Self::format_shutter_speed(shutter_speed)
            }
            _ => "- s".to_string(),
        }
    }

    fn format_rational_value(rational: &Rational) -> String {
        if rational.denom == 0 {
            return "-".to_string();
        }

        let value = rational.num as f64 / rational.denom as f64;
        if (value - value.round()).abs() < 0.05 {
            format!("{:.0}", value)
        } else {
            format!("{:.1}", value)
        }
    }

    fn format_shutter_speed(rational: &Rational) -> String {
        if rational.denom == 0 {
            return "- s".to_string();
        }

        if rational.num >= rational.denom {
            return format!("{} s", Self::format_rational_value(rational));
        }

        let divisor = Self::gcd(rational.num.abs(), rational.denom.abs()).max(1);
        format!("{}/{} s", rational.num / divisor, rational.denom / divisor)
    }

    fn gcd(mut left: i32, mut right: i32) -> i32 {
        while right != 0 {
            let remainder = left % right;
            left = right;
            right = remainder;
        }

        left.abs()
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
