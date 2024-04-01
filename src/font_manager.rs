use std::{collections::HashSet, ffi::OsStr, path::PathBuf, sync::Arc};

use egui::{text::Fonts, FontDefinitions, FontFamily, FontId};
use font_kit::source::SystemSource;
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
}

impl FontManager {
    pub fn new() -> Self {
        Self {
            fonts: IndexMap::new(),
            loading_state: LoadingState::NotLoaded,
        }
    }

    pub fn load_fonts(&mut self, ctx: &egui::Context) {
        if self.loading_state == LoadingState::NotLoaded {
            self.loading_state = LoadingState::Loading;

            let source: SystemSource = SystemSource::new();
            let fonts = source.all_fonts().unwrap();

            for handle in fonts {
                match &handle {
                    font_kit::handle::Handle::Path {
                        path,
                        font_index: _,
                    } => {
                        if path.extension() != Some(OsStr::new("ttf"))
                            && path.extension() != Some(OsStr::new("otf"))
                        {
                            continue;
                        }
                        match handle.load() {
                            Ok(loaded_font) => {
                                let family = loaded_font.family_name().to_string();
                                let weight = loaded_font.properties().weight.0 as u16;
                                let full_name = loaded_font.full_name().to_string();
                                let font_info = FontInfo {
                                    family: family.clone(),
                                    weight,
                                    weighted_name: format!("{}-{}", family, weight),
                                    full_name,
                                    file_path: path.clone(),
                                };
                                self.fonts.entry(family).or_default().push(font_info);
                            }
                            Err(err) => {
                                log::error!("Failed to load font: {:?}", err);
                            }
                        }
                    }
                    font_kit::handle::Handle::Memory {
                        bytes: _,
                        font_index: _,
                    } => {}
                }
            }

            let mut font_definitions = egui::FontDefinitions::default();

            for (_family, fonts) in &self.fonts {
                for font in fonts {
                    match std::fs::read(&font.file_path) {
                        Ok(font_data) => {
                            font_definitions
                                .font_data
                                .insert(font.family.clone(), egui::FontData::from_owned(font_data));

                            font_definitions.families.insert(
                                FontFamily::Name(Arc::from(font.family.clone())),
                                vec![font.family.clone()],
                            );
                        }
                        Err(err) => {
                            log::error!("Failed to read font file: {:?}", err);
                        }
                    }
                }
            }

            let fonts = Fonts::new(1.0, 1024, font_definitions.clone());

            let valid_fonts = fonts
                .families()
                .iter()
                .map(|family| FontId::new(20.0, family.clone()))
                .filter(|font_id| fonts.has_glyphs(font_id, "abcdefghijklmnopqrstuvwxyz1234567890"))
                .map(|font_id| font_id.family.to_string())
                .collect::<HashSet<String>>();

            let mut valid_font_definitions = FontDefinitions::default();

            font_definitions
                .font_data
                .iter()
                .filter(|(family, _)| valid_fonts.contains(&family.to_string()))
                .for_each(|(family, font_data)| {
                    valid_font_definitions
                        .font_data
                        .insert(family.clone(), font_data.clone());

                    valid_font_definitions.families.insert(
                        FontFamily::Name(Arc::from(family.clone())),
                        vec![family.clone()],
                    );
                });

            ctx.set_fonts(valid_font_definitions);

            self.loading_state = LoadingState::Loaded;
        }
    }
}

pub struct FontInfo {
    pub family: String,
    pub weight: u16,
    pub weighted_name: String,
    pub full_name: String,
    pub file_path: PathBuf,
}
