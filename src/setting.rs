use std::{collections::HashMap, sync::LazyLock};

use savefile_derive::Savefile;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumDiscriminants, EnumIter};

use crate::setting::SettingCategory::Gallery;

static SETTINGS_BY_CATEGORY: LazyLock<HashMap<SettingCategory, Vec<SettingDiscriminants>>> =
    LazyLock::new(|| {
        let mut categories: HashMap<_, Vec<_>> = SettingCategory::iter()
            .map(|category| (category, Vec::new()))
            .collect();

        let mut settings: Vec<_> = SettingDiscriminants::iter().collect();
        settings.sort_by(|left, right| left.as_ref().cmp(right.as_ref()));

        for setting in settings {
            categories
                .entry(setting.category())
                .or_default()
                .push(setting);
        }

        categories
    });

#[derive(Debug, Clone, Copy, PartialEq, EnumDiscriminants)]
#[strum_discriminants(derive(EnumIter, AsRefStr))]
pub enum Setting {
    ThumbnailScale(f32),
    ThumbnailQuality(ThumbnailQuality),
    DefaultGallerySort(GallerySort),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, EnumIter, AsRefStr, Savefile)]
pub enum ThumbnailQuality {
    Low,
    #[default]
    Standard,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, EnumIter, AsRefStr, Savefile)]
pub enum GallerySort {
    #[default]
    NewestFirst,
    OldestFirst,
    Filename,
}

impl GallerySort {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NewestFirst => "Newest first",
            Self::OldestFirst => "Oldest first",
            Self::Filename => "Filename",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Savefile)]
pub struct Settings {
    pub thumbnail_scale: f32,
    pub thumbnail_quality: ThumbnailQuality,
    pub default_gallery_sort: GallerySort,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            thumbnail_scale: 1.0,
            thumbnail_quality: ThumbnailQuality::Standard,
            default_gallery_sort: GallerySort::NewestFirst,
        }
    }
}

impl Settings {
    pub fn set(&mut self, setting: Setting) {
        match setting {
            Setting::ThumbnailScale(value) => self.thumbnail_scale = value,
            Setting::ThumbnailQuality(value) => self.thumbnail_quality = value,
            Setting::DefaultGallerySort(value) => self.default_gallery_sort = value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter)]
pub enum SettingCategory {
    Gallery,
}

impl SettingCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Gallery => "Gallery",
        }
    }

    pub fn settings(&self) -> &'static [SettingDiscriminants] {
        &SETTINGS_BY_CATEGORY[self]
    }
}

impl SettingDiscriminants {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ThumbnailScale => "Thumbnail scale",
            Self::ThumbnailQuality => "Thumbnail quality",
            Self::DefaultGallerySort => "Default gallery sort",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::ThumbnailScale => "Preferred size of thumbnails in the gallery.",
            Self::ThumbnailQuality => {
                "Preferred thumbnail quality, balancing image detail and cache size."
            }
            Self::DefaultGallerySort => "Preferred order of photos in the gallery.",
        }
    }

    pub fn category(&self) -> SettingCategory {
        match self {
            Self::ThumbnailScale | Self::ThumbnailQuality | Self::DefaultGallerySort => Gallery,
        }
    }
}

impl Setting {
    fn category(&self) -> SettingCategory {
        SettingDiscriminants::from(self).category()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_grouped_and_sorted() {
        let mut count = 0;
        for category in SettingCategory::iter() {
            let settings = category.settings();
            assert!(
                settings
                    .iter()
                    .all(|setting| setting.category() == category)
            );
            assert!(
                settings
                    .windows(2)
                    .all(|pair| pair[0].as_ref() <= pair[1].as_ref())
            );
            count += settings.len();
        }
        assert_eq!(count, SettingDiscriminants::iter().count());
        assert_eq!(
            Gallery.settings(),
            &[
                SettingDiscriminants::DefaultGallerySort,
                SettingDiscriminants::ThumbnailQuality,
                SettingDiscriminants::ThumbnailScale,
            ]
        );
    }

    #[test]
    fn settings_have_defaults_matching_existing_behavior() {
        let settings = Settings::default();
        assert_eq!(settings.thumbnail_scale, 1.0);
        assert_eq!(settings.thumbnail_quality, ThumbnailQuality::Standard);
        assert_eq!(settings.default_gallery_sort, GallerySort::NewestFirst);
    }

    #[test]
    fn setting_updates_preserve_other_values() {
        let mut settings = Settings::default();
        settings.set(Setting::ThumbnailScale(1.25));
        assert_eq!(settings.thumbnail_scale, 1.25);
        assert_eq!(settings.thumbnail_quality, ThumbnailQuality::Standard);
        assert_eq!(settings.default_gallery_sort, GallerySort::NewestFirst);

        settings.set(Setting::ThumbnailQuality(ThumbnailQuality::High));
        assert_eq!(settings.thumbnail_scale, 1.25);
        assert_eq!(settings.thumbnail_quality, ThumbnailQuality::High);
        assert_eq!(settings.default_gallery_sort, GallerySort::NewestFirst);

        settings.set(Setting::DefaultGallerySort(GallerySort::OldestFirst));
        assert_eq!(settings.thumbnail_scale, 1.25);
        assert_eq!(settings.thumbnail_quality, ThumbnailQuality::High);
        assert_eq!(settings.default_gallery_sort, GallerySort::OldestFirst);
    }

    #[test]
    fn settings_reuse_the_cached_list() {
        assert!(std::ptr::eq(Gallery.settings(), Gallery.settings()));
    }
}
