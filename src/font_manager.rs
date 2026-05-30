use std::{collections::HashSet, sync::Arc};

use egui::{FontDefinitions, FontFamily, FontId, epaint::TextOptions, text::Fonts};
use font_kit::source::SystemSource;
use font_kit::{handle::Handle, properties::Style};
use indexmap::IndexMap;

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

            let source: SystemSource = SystemSource::new();
            let fonts = source.all_fonts().unwrap();

            for handle in fonts {
                match Self::font_info_from_handle(&handle) {
                    Some(font_info) => {
                        self.fonts
                            .entry(font_info.family.clone())
                            .or_default()
                            .push(font_info);
                    }
                    None => log::error!("Failed to load font: {:?}", handle),
                }
            }

            let mut font_definitions = egui::FontDefinitions::default();

            for (family, fonts) in &mut self.fonts {
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

            ctx.set_fonts(valid_font_definitions.clone());

            self.font_definitions = Some(Arc::new(valid_font_definitions));
            self.loading_state = LoadingState::Loaded;
        }
    }

    fn font_info_from_handle(handle: &Handle) -> Option<FontInfo> {
        let loaded_font = handle
            .load()
            .map_err(|err| {
                log::error!("Failed to load font: {:?}", err);
                err
            })
            .ok()?;

        let font_index = match handle {
            Handle::Path { font_index, .. } | Handle::Memory { font_index, .. } => *font_index,
        };

        let font_bytes = match handle {
            Handle::Path { path, .. } => std::fs::read(path)
                .map_err(|err| {
                    log::error!("Failed to read font file {:?}: {:?}", path, err);
                    err
                })
                .ok()?,
            Handle::Memory { bytes, .. } => (**bytes).clone(),
        };

        let family = loaded_font.family_name().to_string();
        let properties = loaded_font.properties();
        let weight = properties.weight.0 as u16;
        let style = properties.style;
        let full_name = loaded_font.full_name().to_string();
        let font_data_name = loaded_font
            .postscript_name()
            .unwrap_or_else(|| format!("{}-{}-{}-{}", family, full_name, weight, font_index));
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

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn loads_macos_system_fonts() {
        let ctx = egui::Context::default();
        let mut font_manager = FontManager::new();

        font_manager.load_fonts(&ctx);

        assert!(!font_manager.fonts.is_empty());
        let font_definitions = font_manager.font_definitions.as_ref().unwrap();
        assert!(
            font_definitions
                .families
                .keys()
                .any(|family| matches!(family, FontFamily::Name(_)))
        );
    }
}
