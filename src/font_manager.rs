use std::{collections::HashSet, sync::Arc};

use egui::{FontDefinitions, FontFamily, FontId, epaint::TextOptions, text::Fonts};
use font_kit::source::SystemSource;
use font_kit::{handle::Handle, properties::Style};
use indexmap::IndexMap;

use crate::dep_mut;

#[derive(Debug, PartialEq)]
pub enum LoadingState {
    NotLoaded,
    Loading,
    Loaded,
}

pub struct FontManager {
    pub fonts: IndexMap<String, Vec<FontInfo>>,
    pub loading_state: LoadingState,
    pub font_definitions: Option<Arc<FontDefinitions>>,
}

impl FontManager {
    pub fn new() -> Self {
        Self {
            fonts: IndexMap::new(),
            loading_state: LoadingState::NotLoaded,
            font_definitions: None,
        }
    }

    pub fn load_fonts(&mut self, ctx: &egui::Context) {
        if self.loading_state == LoadingState::NotLoaded {
            self.loading_state = LoadingState::Loading;
            let ctx = ctx.clone();

            tokio::spawn(async move {
                match Self::load_font_definitions().await {
                    Some((fonts, font_definitions)) => {
                        ctx.set_fonts(font_definitions.clone());
                        dep_mut!(FontManager, |font_manager| {
                            font_manager.fonts = fonts;
                            font_manager.font_definitions = Some(Arc::new(font_definitions));
                            font_manager.loading_state = LoadingState::Loaded;
                        });
                    }
                    None => {
                        dep_mut!(FontManager, |font_manager| {
                            font_manager.loading_state = LoadingState::NotLoaded;
                        });
                    }
                }
            });
        }
    }

    async fn load_font_definitions() -> Option<(IndexMap<String, Vec<FontInfo>>, FontDefinitions)> {
        let handles = {
            let source: SystemSource = SystemSource::new();
            source.all_fonts().unwrap()
        };
        let mut font_infos: IndexMap<String, Vec<FontInfo>> = IndexMap::new();

        for handle in handles {
            match Self::font_info_from_handle(&handle).await {
                Some(font_info) => {
                    font_infos
                        .entry(font_info.family.clone())
                        .or_default()
                        .push(font_info);
                }
                None => log::debug!("Skipped font: {:?}", handle),
            }
        }

        let mut font_definitions = egui::FontDefinitions::default();

        for (family, fonts) in &mut font_infos {
            fonts.sort_by_key(|font| font.sort_key());

            let font_names = fonts
                .iter()
                .map(|font| font.font_data_name.clone())
                .collect::<Vec<_>>();

            for font in fonts {
                font_definitions
                    .font_data
                    .insert(font.font_data_name.clone(), font.font_data.clone());
            }

            font_definitions
                .families
                .insert(FontFamily::Name(Arc::from(family.clone())), font_names);
        }

        let mut fonts = Fonts::new(TextOptions::default(), font_definitions.clone());

        let valid_fonts = fonts
            .with_pixels_per_point(1.0)
            .families()
            .iter()
            .map(|family| FontId::new(20.0, family.clone()))
            .filter(|font_id| fonts.has_glyphs(font_id, "abcdefghijklmnopqrstuvwxyz1234567890"))
            .map(|font_id| font_id.family.to_string())
            .collect::<HashSet<String>>();

        let mut valid_font_definitions = FontDefinitions::default();

        font_definitions
            .families
            .iter()
            .for_each(|(family, font_names)| {
                if !valid_fonts.contains(&family.to_string()) {
                    return;
                }

                let valid_font_names = font_names
                    .iter()
                    .filter(|font_name| font_definitions.font_data.contains_key(*font_name))
                    .cloned()
                    .collect::<Vec<_>>();

                if valid_font_names.is_empty() {
                    return;
                }

                valid_font_names.iter().for_each(|font_name| {
                    if let Some(font_data) = font_definitions.font_data.get(font_name) {
                        valid_font_definitions
                            .font_data
                            .insert(font_name.clone(), font_data.clone());
                    }
                });

                valid_font_definitions
                    .families
                    .insert(family.clone(), valid_font_names);
            });

        Some((font_infos, valid_font_definitions))
    }

    async fn font_info_from_handle(handle: &Handle) -> Option<FontInfo> {
        let raw_font_index = match handle {
            Handle::Path { font_index, .. } | Handle::Memory { font_index, .. } => *font_index,
        };
        // Fontconfig stores named-instance bits above the face index; egui only accepts the
        // sfnt collection face index.
        let font_index = raw_font_index & 0xffff;

        let font_bytes = match handle {
            Handle::Path { path, .. } => tokio::fs::read(path)
                .await
                .map_err(|err| {
                    log::error!("Failed to read font file {:?}: {:?}", path, err);
                    err
                })
                .ok()?,
            Handle::Memory { bytes, .. } => (**bytes).clone(),
        };
        if !Self::is_supported_egui_font_data(&font_bytes) {
            log::debug!("Skipping unsupported system font: {:?}", handle);
            return None;
        }

        let loaded_font = handle
            .load()
            .map_err(|err| {
                log::error!("Failed to load font: {:?}", err);
                err
            })
            .ok()?;

        let family = loaded_font.family_name().to_string();
        let properties = loaded_font.properties();
        let weight = properties.weight.0 as u16;
        let style = properties.style;
        let full_name = loaded_font.full_name().to_string();
        let font_data_name = loaded_font
            .postscript_name()
            .unwrap_or_else(|| format!("{}-{}-{}-{}", family, full_name, weight, raw_font_index));
        let mut font_data = egui::FontData::from_owned(font_bytes);
        font_data.index = font_index;

        Some(FontInfo {
            family: family.clone(),
            weight,
            style,
            weighted_name: format!("{}-{}", family, weight),
            full_name,
            font_data_name,
            font_data: Arc::new(font_data),
        })
    }

    fn is_supported_egui_font_data(bytes: &[u8]) -> bool {
        matches!(
            bytes.get(..4),
            Some(b"\x00\x01\x00\x00" | b"OTTO" | b"true" | b"ttcf")
        )
    }

    pub fn available_font_id(
        ctx: &egui::Context,
        font_id: &egui::FontId,
        font_size: f32,
    ) -> egui::FontId {
        let family = ctx.fonts(|fonts| {
            if fonts.families().contains(&font_id.family) {
                font_id.family.clone()
            } else {
                egui::FontFamily::Proportional
            }
        });

        egui::FontId::new(font_size, family)
    }
}

pub struct FontInfo {
    pub family: String,
    pub weight: u16,
    pub style: Style,
    #[allow(dead_code)]
    pub weighted_name: String,
    #[allow(dead_code)]
    pub full_name: String,
    pub font_data_name: String,
    pub font_data: Arc<egui::FontData>,
}

impl FontInfo {
    fn sort_key(&self) -> (u8, u16) {
        let style_priority = match self.style {
            Style::Normal => 0,
            Style::Italic => 1,
            Style::Oblique => 2,
        };

        let weight_distance = self.weight.abs_diff(400);

        (style_priority, weight_distance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_egui_supported_font_containers() {
        assert!(FontManager::is_supported_egui_font_data(
            b"\x00\x01\x00\x00..."
        ));
        assert!(FontManager::is_supported_egui_font_data(b"OTTO..."));
        assert!(FontManager::is_supported_egui_font_data(b"true..."));
        assert!(FontManager::is_supported_egui_font_data(b"ttcf..."));
        assert!(!FontManager::is_supported_egui_font_data(
            b"\x80\x01k\x03%!PS-Adobe"
        ));
        assert!(!FontManager::is_supported_egui_font_data(b""));
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn loads_linux_system_fonts_without_panicking_on_type1_fonts() {
        let result = FontManager::load_font_definitions().await;

        assert!(result.is_some());
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn loads_macos_system_fonts() {
        let (fonts, font_definitions) = FontManager::load_font_definitions().await.unwrap();

        assert!(!fonts.is_empty());
        assert!(
            font_definitions
                .families
                .keys()
                .any(|family| matches!(family, FontFamily::Name(_)))
        );
    }
}
