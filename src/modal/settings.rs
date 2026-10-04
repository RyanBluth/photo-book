use egui::{Align, Button, ComboBox, Layout, ScrollArea, Slider, TextEdit, Ui, Widget, vec2};
use strum::IntoEnumIterator;

use crate::{
    auto_persisting::AutoPersisting,
    config::{Config, ConfigModification},
    dep, dep_mut,
    setting::{
        GallerySort, Setting, SettingCategory, SettingDiscriminants, Settings, ThumbnailQuality,
    },
    theme::color,
};

use super::{Modal, ModalActionResponse};

#[derive(Debug, Clone)]
pub struct SettingsModal {
    search: String,
    category: SettingCategory,
    save_error: Option<String>,
}

impl Default for SettingsModal {
    fn default() -> Self {
        Self {
            search: String::new(),
            category: SettingCategory::Gallery,
            save_error: None,
        }
    }
}

impl SettingsModal {
    pub fn new() -> Self {
        Self::default()
    }

    fn sidebar_ui(&mut self, ui: &mut Ui) {
        ui.add(
            TextEdit::singleline(&mut self.search)
                .hint_text("Search settings...")
                .desired_width(f32::INFINITY),
        );
        ui.add_space(12.0);
        for category in SettingCategory::iter() {
            if ui
                .add_sized(
                    [ui.available_width(), 32.0],
                    Button::selectable(self.category == category, category.label()),
                )
                .clicked()
            {
                self.category = category;
            }
        }
    }

    fn content_ui(&self, ui: &mut Ui, settings: &Settings) -> Vec<Setting> {
        ui.weak("User");
        ui.add_space(16.0);
        ui.heading(self.category.label());
        ui.add_space(12.0);
        ui.weak(format!("{} Settings", self.category.label()));
        ui.separator();
        ui.add_space(12.0);

        let query = self.search.trim().to_lowercase();
        let mut matched = false;
        let mut changes = Vec::new();
        for &setting in self.category.settings() {
            if !matches_search(setting, &query) {
                continue;
            }
            matched = true;
            if let Some(change) = setting_row_ui(ui, setting, settings) {
                changes.push(change);
            }
            ui.add_space(16.0);
            ui.separator();
            ui.add_space(12.0);
        }
        if !matched {
            ui.weak("No settings match your search.");
        }
        if let Some(error) = &self.save_error {
            ui.colored_label(color::ERROR, format!("Could not save settings: {error}"));
        }
        changes
    }

    fn show(
        &mut self,
        ui: &mut Ui,
        settings: Result<&Settings, &str>,
    ) -> (egui::Response, Vec<Setting>) {
        let screen_size = ui.ctx().content_rect().size();
        let width = (screen_size.x - 80.0).clamp(400.0, 900.0);
        let height = (screen_size.y * 0.8 - 120.0).max(160.0);
        ui.set_width(width);
        let mut changes = Vec::new();

        let response = ui.horizontal_top(|ui| {
            // Bound the separator's fill height independently of the modal's last size.
            ui.set_height(height);
            ui.allocate_ui_with_layout(vec2(180.0, height), Layout::top_down(Align::Min), |ui| {
                ui.set_width(180.0);
                ui.set_min_height(height);
                self.sidebar_ui(ui);
            });
            ui.separator();
            ui.add_space(12.0);
            ui.allocate_ui_with_layout(
                vec2(ui.available_width(), height),
                Layout::top_down(Align::Min),
                |ui| {
                    ScrollArea::vertical()
                        .id_salt("settings_content")
                        .max_height(height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| match settings {
                            Ok(settings) => changes = self.content_ui(ui, settings),
                            Err(error) => {
                                ui.colored_label(
                                    color::ERROR,
                                    format!("Could not load settings: {error}"),
                                );
                            }
                        });
                },
            );
        });
        (response.response, changes)
    }
}

fn matches_search(setting: SettingDiscriminants, query: &str) -> bool {
    let text = format!(
        "{} {} {}",
        setting.category().label(),
        setting.label(),
        setting.description(),
    );
    text.to_lowercase().contains(query)
}

fn setting_row_ui(
    ui: &mut Ui,
    setting: SettingDiscriminants,
    settings: &Settings,
) -> Option<Setting> {
    egui::Sides::new()
        .shrink_left()
        .wrap()
        .spacing(24.0)
        .show(
            ui,
            |ui| {
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width().min(320.0));
                    ui.strong(setting.label());
                    ui.add_space(4.0);
                    ui.weak(setting.description());
                });
            },
            |ui| setting_control_ui(ui, setting, settings),
        )
        .1
}

fn setting_control_ui(
    ui: &mut Ui,
    setting: SettingDiscriminants,
    settings: &Settings,
) -> Option<Setting> {
    match setting {
        SettingDiscriminants::ThumbnailScale => {
            let mut value = settings.thumbnail_scale;
            ui.spacing_mut().slider_width = 120.0;
            ui.add(Slider::new(&mut value, 0.5..=1.5).step_by(0.05))
                .changed()
                .then_some(Setting::ThumbnailScale(value))
        }
        SettingDiscriminants::ThumbnailQuality => {
            let mut value = settings.thumbnail_quality;
            ComboBox::from_id_salt("thumbnail_quality")
                .width(160.0)
                .selected_text(value.as_ref())
                .show_ui(ui, |ui| {
                    for quality in ThumbnailQuality::iter() {
                        ui.selectable_value(&mut value, quality, quality.as_ref());
                    }
                });
            (value != settings.thumbnail_quality).then_some(Setting::ThumbnailQuality(value))
        }
        SettingDiscriminants::DefaultGallerySort => {
            let mut value = settings.default_gallery_sort;
            ComboBox::from_id_salt("default_gallery_sort")
                .width(160.0)
                .selected_text(value.label())
                .show_ui(ui, |ui| {
                    for sort in GallerySort::iter() {
                        ui.selectable_value(&mut value, sort, sort.label());
                    }
                });
            (value != settings.default_gallery_sort).then_some(Setting::DefaultGallerySort(value))
        }
    }
}

impl Widget for &mut SettingsModal {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        // Release the config lock before drawing widgets or saving changes.
        let settings = dep!(AutoPersisting<Config>, |config| {
            config
                .read()
                .map(|config| config.settings().clone())
                .map_err(|error| error.to_string())
        });
        let (response, changes) = self.show(ui, settings.as_ref().map_err(String::as_str));
        for change in changes {
            let result = dep_mut!(AutoPersisting<Config>, |config| {
                config.modify(ConfigModification::SetSetting(change))
            });
            self.save_error = result.err().map(|error| error.to_string());
            ui.ctx().request_repaint();
            if self.save_error.is_some() {
                break;
            }
        }
        response
    }
}

impl Modal for SettingsModal {
    type Response = ModalActionResponse;

    fn title(&self) -> String {
        "Settings".to_string()
    }

    fn body_ui(&mut self, ui: &mut Ui) {
        ui.add(self);
    }

    fn actions_ui(&mut self, ui: &mut Ui) -> Option<Self::Response> {
        ui.button("Close")
            .clicked()
            .then_some(ModalActionResponse::_Close)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_matches_category_labels_and_descriptions() {
        let settings = SettingCategory::Gallery.settings();
        assert!(settings.iter().all(|&setting| matches_search(setting, "")));
        assert!(
            settings
                .iter()
                .all(|&setting| matches_search(setting, "gallery"))
        );
        let thumbnails: Vec<_> = settings
            .iter()
            .copied()
            .filter(|&setting| matches_search(setting, "thumbnail"))
            .collect();
        assert_eq!(
            thumbnails,
            vec![
                SettingDiscriminants::ThumbnailQuality,
                SettingDiscriminants::ThumbnailScale
            ]
        );
        assert!(matches_search(
            SettingDiscriminants::ThumbnailQuality,
            "cache size"
        ));
        assert!(
            !settings
                .iter()
                .any(|&setting| matches_search(setting, "nonexistent"))
        );
    }

    #[test]
    fn rendering_does_not_change_saved_settings() {
        let context = egui::Context::default();
        let mut modal = SettingsModal::new();
        let settings = Settings {
            thumbnail_scale: 1.25,
            thumbnail_quality: ThumbnailQuality::High,
            default_gallery_sort: GallerySort::Filename,
        };
        for search in ["", "THUMBNAIL", "nonexistent"] {
            modal.search = search.into();
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                assert!(modal.show(ui, Ok(&settings)).1.is_empty());
            });
            output.textures_delta.clear();
        }
    }

    #[test]
    fn load_errors_do_not_produce_setting_updates() {
        let context = egui::Context::default();
        let mut modal = SettingsModal::new();
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            assert!(modal.show(ui, Err("Invalid config")).1.is_empty());
        });
        output.textures_delta.clear();
    }

    #[test]
    fn modal_height_stays_stable_across_frames() {
        let context = egui::Context::default();
        let mut modal = SettingsModal::new();
        let settings = Settings::default();
        let mut heights = Vec::new();
        for _ in 0..20 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(1280.0, 900.0),
                )),
                ..Default::default()
            };
            let mut output = context.run_ui(input, |ui| {
                let response =
                    egui::Modal::new(egui::Id::new("settings_height_test")).show(ui.ctx(), |ui| {
                        ui.heading("Settings");
                        assert!(modal.show(ui, Ok(&settings)).1.is_empty());
                        let _ = ui.button("Close");
                    });
                heights.push(response.response.rect.height());
            });
            output.textures_delta.clear();
        }
        assert!(
            heights[19] < 900.0,
            "Modal exceeds window height: {heights:?}"
        );
        assert!(
            heights[5..]
                .windows(2)
                .all(|pair| (pair[0] - pair[1]).abs() < 1.0),
            "Modal grows between frames: {heights:?}"
        );
    }
}
