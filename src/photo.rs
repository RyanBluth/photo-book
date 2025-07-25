use std::{
    collections::{HashMap, HashSet},
    f32::consts::PI,
    fmt::Display,
    fs::File,
    hash::{Hash, Hasher},
    io::BufReader,
    ops::{Deref, DerefMut},
    path::PathBuf,
};
use tokio::fs::File as TokioFile;

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    dirs::Dirs,
    photo_manager::PhotoManager,
    utils::ExifDateTimeExt,
};

use eframe::{
    emath::Rot2,
    epaint::{Pos2, Rect, Vec2},
};

use chrono::{DateTime, Utc};
use exif::{In, Reader, Tag, Value};
use fxhash::hash64;
use log::error;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

macro_rules! metadata_fields {
    ($(($name:ident, $type:ty)),*) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum PhotoMetadataField {
            $( $name($type), )*
        }

        #[derive(Debug, Clone, Eq, PartialEq, Hash, Copy, Serialize, Deserialize)]
        pub enum PhotoMetadataFieldLabel {
            $( $name, )*
        }

        impl PhotoMetadataField {
            pub fn label(&self) -> PhotoMetadataFieldLabel {
                match self {
                    $( Self::$name(_) => PhotoMetadataFieldLabel::$name, )*
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PhotoError {
    #[error("Failed to load photo: {0}")]
    LoadError(#[from] std::io::Error),
}

pub enum MaxPhotoDimension {
    Width,
    Height,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PhotoRotation {
    Normal,
    MirrorHorizontal,
    Rotate180,
    MirrorVerticalAndRotate180,
    MirrorHorizontalAndRotate90CW,
    Rotate90CW,
    MirrorHorizontalAndRotate270CW,
    Rotate270CW,
}

// = 0 degrees: the correct orientation, no adjustment is required.
// = 0 degrees, mirrored: image has been flipped back-to-front.
// = 180 degrees: image is upside down.
// = 180 degrees, mirrored: image has been flipped back-to-front and is upside down.
// = 90 degrees: image has been flipped back-to-front and is on its side.
// = 90 degrees, mirrored: image is on its side.
// = 270 degrees: image has been flipped back-to-front and is on its far side.
// = 270 degrees, mirrored: image is on its far side.

impl PhotoRotation {
    pub fn radians(&self) -> f32 {
        match self {
            Self::Normal => 0.0,
            Self::MirrorHorizontal => 0.0,
            Self::Rotate180 => PI,
            Self::MirrorVerticalAndRotate180 => PI,
            Self::MirrorHorizontalAndRotate90CW => PI / 2.0,
            Self::Rotate90CW => PI / 2.0,
            Self::MirrorHorizontalAndRotate270CW => (3.0 * PI) / 2.0,
            Self::Rotate270CW => (3.0 * PI) / 2.0,
        }
    }

    pub fn is_horizontal(&self) -> bool {
        match self {
            Self::Normal => true,
            Self::MirrorHorizontal => true,
            Self::Rotate180 => true,
            Self::MirrorVerticalAndRotate180 => true,
            Self::MirrorHorizontalAndRotate90CW => false,
            Self::Rotate90CW => false,
            Self::MirrorHorizontalAndRotate270CW => false,
            Self::Rotate270CW => false,
        }
    }
}

impl Display for PhotoRotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => f.write_str("Normal"),
            Self::MirrorHorizontal => f.write_str("Mirror Horizontal"),
            Self::Rotate180 => f.write_str("Rotate 180"),
            Self::MirrorVerticalAndRotate180 => f.write_str("Mirror Vertical and Rotate 180"),
            Self::MirrorHorizontalAndRotate90CW => {
                f.write_str("Mirror Horizontal and Rotate 90 CW")
            }
            Self::Rotate90CW => f.write_str("Rotate 90 CW"),
            Self::MirrorHorizontalAndRotate270CW => {
                f.write_str("Mirror Horizontal and Rotate 270 CW")
            }
            Self::Rotate270CW => f.write_str("Rotate 270 CW"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataCollection {
    fields: HashMap<PhotoMetadataFieldLabel, PhotoMetadataField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rational {
    pub num: i32,
    pub denom: i32,
}

impl MetadataCollection {
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }

    pub fn insert(&mut self, field: PhotoMetadataField) {
        self.fields.insert(field.label(), field);
    }

    pub fn get(&self, label: PhotoMetadataFieldLabel) -> Option<&PhotoMetadataField> {
        self.fields.get(&label)
    }
}

metadata_fields!(
    (Path, PathBuf),
    (Width, usize),
    (Height, usize),
    (Rotation, PhotoRotation),
    (RotatedWidth, usize),
    (RotatedHeight, usize),
    (Camera, String),
    (DateTime, DateTime<Utc>),
    (ISO, u32),
    (ShutterSpeed, Rational),
    (Aperture, Rational),
    (FocalLength, Rational)
);

impl Display for PhotoMetadataField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhotoMetadataField::Path(path) => f.write_str(&path.display().to_string()),
            PhotoMetadataField::Width(width) => write!(f, "{}px", width),
            PhotoMetadataField::Height(height) => write!(f, "{}px", height),
            PhotoMetadataField::Rotation(rotation) => write!(f, "{}", rotation),
            PhotoMetadataField::RotatedWidth(rotated_width) => {
                write!(f, "{}px", rotated_width)
            }
            PhotoMetadataField::RotatedHeight(rotated_height) => {
                write!(f, "{}px", rotated_height)
            }
            PhotoMetadataField::Camera(camera) => write!(f, "{}", camera),
            PhotoMetadataField::DateTime(date_time) => write!(f, "{}", date_time),
            PhotoMetadataField::ISO(iso) => write!(f, "{}", iso),
            PhotoMetadataField::ShutterSpeed(shutter_speed) => {
                write!(f, "{}/{} sec.", shutter_speed.num, shutter_speed.denom)
            }
            PhotoMetadataField::Aperture(aperture) => {
                write!(f, "f/{}", aperture.num / aperture.denom)
            }
            PhotoMetadataField::FocalLength(focal_length) => {
                write!(f, "{}mm", focal_length.num / focal_length.denom)
            }
        }
    }
}

impl Display for PhotoMetadataFieldLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhotoMetadataFieldLabel::Path => f.write_str("Path"),
            PhotoMetadataFieldLabel::Width => f.write_str("Width"),
            PhotoMetadataFieldLabel::Height => f.write_str("Height"),
            PhotoMetadataFieldLabel::Rotation => f.write_str("Rotation"),
            PhotoMetadataFieldLabel::RotatedWidth => f.write_str("Rotated Width"),
            PhotoMetadataFieldLabel::RotatedHeight => f.write_str("Rotated Height"),
            PhotoMetadataFieldLabel::Camera => f.write_str("Camera"),
            PhotoMetadataFieldLabel::DateTime => f.write_str("Date/Time"),
            PhotoMetadataFieldLabel::ISO => f.write_str("ISO"),
            PhotoMetadataFieldLabel::ShutterSpeed => f.write_str("Shutter Speed"),
            PhotoMetadataFieldLabel::Aperture => f.write_str("Aperture"),
            PhotoMetadataFieldLabel::FocalLength => f.write_str("Focal Length"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoMetadata {
    pub fields: MetadataCollection,
}

impl PhotoMetadata {
    fn process_metadata(
        path: &PathBuf,
        exif: Result<exif::Exif, exif::Error>,
        size: imagesize::ImageSize,
    ) -> MetadataCollection {
        let mut fields = MetadataCollection::new();
        fields.insert(PhotoMetadataField::Path(path.clone()));

        let width = size.width;
        let height = size.height;

        fields.insert(PhotoMetadataField::Width(width));
        fields.insert(PhotoMetadataField::Height(height));

        if let Ok(exif) = exif {
            if let Some(field) = exif.get_field(Tag::Orientation, In::PRIMARY) {
                let mut rotation = PhotoRotation::Normal;
                if let Some(value) = field.value.get_uint(0) {
                    match value {
                        1 => {
                            // Normal
                            rotation = PhotoRotation::Normal;
                        }
                        2 => {
                            // Mirror horizontal
                            rotation = PhotoRotation::MirrorHorizontal;
                        }
                        3 => {
                            // Rotate 180
                            rotation = PhotoRotation::Rotate180;
                        }
                        4 => {
                            // Mirror vertical
                            rotation = PhotoRotation::MirrorVerticalAndRotate180;
                        }
                        5 => {
                            // Mirror horizontal and rotate 270 CW
                            rotation = PhotoRotation::MirrorHorizontalAndRotate270CW;
                        }
                        6 => {
                            // Rotate 90 CW
                            rotation = PhotoRotation::Rotate90CW;
                        }
                        7 => {
                            // Mirror horizontal and rotate 90 CW
                            rotation = PhotoRotation::MirrorHorizontalAndRotate90CW;
                        }
                        8 => {
                            // Rotate 270 CW
                            rotation = PhotoRotation::Rotate270CW;
                        }
                        _ => {
                            // Unknown
                        }
                    }
                }
                fields.insert(PhotoMetadataField::Rotation(rotation));

                let rect = Rect::from_min_size(
                    Pos2::new(0.0, 0.0),
                    Vec2::new(width as f32, height as f32),
                );
                let rotated_size = rect.rotate_bb(Rot2::from_angle(rotation.radians())).size();

                fields.insert(PhotoMetadataField::RotatedWidth(rotated_size.x as usize));
                fields.insert(PhotoMetadataField::RotatedHeight(rotated_size.y as usize));
            } else {
                fields.insert(PhotoMetadataField::Rotation(PhotoRotation::Normal));
                fields.insert(PhotoMetadataField::RotatedWidth(width));
                fields.insert(PhotoMetadataField::RotatedHeight(height));
            }

            if let Some(field) = exif.get_field(Tag::Model, In::PRIMARY) {
                if let Value::Ascii(ref vec) = field.value {
                    if let Some(value) = vec.first() {
                        fields.insert(PhotoMetadataField::Camera(
                            String::from_utf8_lossy(value).to_string(),
                        ));
                    }
                }
            };
            if let Some(field) = exif.get_field(Tag::DateTimeOriginal, In::PRIMARY) {
                if let Value::Ascii(ref vec) = field.value {
                    if let Some(date_time) = vec
                        .first()
                        .and_then(|value| exif::DateTime::from_ascii(value).ok())
                        .and_then(|exif_date_time| exif_date_time.into_chrono_date_time().ok())
                    {
                        fields.insert(PhotoMetadataField::DateTime(date_time))
                    }
                }
            }

            if let Some(field) = exif.get_field(Tag::PhotographicSensitivity, In::PRIMARY) {
                if let Some(value) = field.value.get_uint(0) {
                    fields.insert(PhotoMetadataField::ISO(value));
                }
            }

            if let Some(field) = exif.get_field(Tag::ExposureTime, In::PRIMARY) {
                match field.value {
                    Value::Rational(ref vec) => {
                        if let Some(value) = vec.first() {
                            fields.insert(PhotoMetadataField::ShutterSpeed(Rational {
                                num: value.num as i32,
                                denom: value.denom as i32,
                            }));
                        }
                    }
                    Value::SRational(ref vec) => {
                        if let Some(value) = vec.first() {
                            fields.insert(PhotoMetadataField::ShutterSpeed(Rational {
                                num: value.num,
                                denom: value.denom,
                            }));
                        }
                    }
                    _ => {}
                }
            }

            if let Some(field) = exif.get_field(Tag::FNumber, In::PRIMARY) {
                if let Value::Rational(ref vec) = field.value {
                    if let Some(value) = vec.first() {
                        fields.insert(PhotoMetadataField::Aperture(Rational {
                            num: value.num as i32,
                            denom: value.denom as i32,
                        }));
                    }
                }
            }

            if let Some(field) = exif.get_field(Tag::FocalLength, In::PRIMARY) {
                if let Value::Rational(ref vec) = field.value {
                    if let Some(value) = vec.first() {
                        fields.insert(PhotoMetadataField::FocalLength(Rational {
                            num: value.num as i32,
                            denom: value.denom as i32,
                        }));
                    }
                }
            }
        } else {
            fields.insert(PhotoMetadataField::Rotation(PhotoRotation::Normal));
            fields.insert(PhotoMetadataField::RotatedWidth(width));
            fields.insert(PhotoMetadataField::RotatedHeight(height));
        }

        fields
    }

    pub fn from_path(path: &PathBuf) -> Result<Self, PhotoError> {
        let file = File::open(path)?;
        let exif = Reader::new().read_from_container(&mut BufReader::new(&file));
        let size = imagesize::size(path.clone()).unwrap_or(imagesize::ImageSize {
            width: 0,
            height: 0,
        });

        Ok(Self {
            fields: Self::process_metadata(path, exif, size),
        })
    }

    pub async fn from_path_async(path: &PathBuf) -> Result<Self, PhotoError> {
        let file = TokioFile::open(path).await?;
        let exif = Reader::new().read_from_container(&mut BufReader::new(file.into_std().await));
        let size = imagesize::size(path.clone()).unwrap_or(imagesize::ImageSize {
            width: 0,
            height: 0,
        });

        Ok(Self {
            fields: Self::process_metadata(path, exif, size),
        })
    }

    pub fn width(&self) -> usize {
        match self.fields.get(PhotoMetadataFieldLabel::Width) {
            Some(PhotoMetadataField::Width(width)) => *width,
            _ => 0,
        }
    }

    pub fn height(&self) -> usize {
        match self.fields.get(PhotoMetadataFieldLabel::Height) {
            Some(PhotoMetadataField::Height(height)) => *height,
            _ => 0,
        }
    }

    pub fn rotation(&self) -> PhotoRotation {
        match self.fields.get(PhotoMetadataFieldLabel::Rotation) {
            Some(PhotoMetadataField::Rotation(rotation)) => *rotation,
            _ => PhotoRotation::Normal,
        }
    }

    pub fn rotated_width(&self) -> usize {
        match self.fields.get(PhotoMetadataFieldLabel::RotatedWidth) {
            Some(PhotoMetadataField::RotatedWidth(rotated_width)) => *rotated_width,
            _ => 0,
        }
    }

    pub fn rotated_height(&self) -> usize {
        match self.fields.get(PhotoMetadataFieldLabel::RotatedHeight) {
            Some(PhotoMetadataField::RotatedHeight(rotated_height)) => *rotated_height,
            _ => 0,
        }
    }

    pub fn does_rotation_alter_dimensions(&self) -> bool {
        match self.fields.get(PhotoMetadataFieldLabel::Rotation) {
            Some(PhotoMetadataField::Rotation(rotation)) => match rotation {
                PhotoRotation::Normal => false,
                PhotoRotation::MirrorHorizontal => false,
                PhotoRotation::Rotate180 => false,
                PhotoRotation::MirrorVerticalAndRotate180 => false,
                PhotoRotation::MirrorHorizontalAndRotate90CW => true,
                PhotoRotation::Rotate90CW => true,
                PhotoRotation::MirrorHorizontalAndRotate270CW => true,
                PhotoRotation::Rotate270CW => true,
            },
            _ => false,
        }
    }

    pub fn get(&self, label: PhotoMetadataFieldLabel) -> Option<&PhotoMetadataField> {
        self.fields.get(label)
    }

    pub fn iter(&self) -> impl Iterator<Item = (PhotoMetadataFieldLabel, &PhotoMetadataField)> {
        vec![
            PhotoMetadataFieldLabel::Path,
            PhotoMetadataFieldLabel::Width,
            PhotoMetadataFieldLabel::Height,
            PhotoMetadataFieldLabel::Rotation,
            PhotoMetadataFieldLabel::RotatedWidth,
            PhotoMetadataFieldLabel::RotatedHeight,
            PhotoMetadataFieldLabel::Camera,
            PhotoMetadataFieldLabel::DateTime,
            PhotoMetadataFieldLabel::ISO,
            PhotoMetadataFieldLabel::ShutterSpeed,
            PhotoMetadataFieldLabel::Aperture,
            PhotoMetadataFieldLabel::FocalLength,
        ]
        .into_iter()
        .filter_map(|label| self.fields.get(label).map(|value| (label, value)))
    }
}

#[derive(Debug, Clone, Copy, EnumIter, PartialEq, Eq, Hash)]
pub enum PhotoRating {
    Yes = 0,
    Maybe = 1,
    No = 2,
}

impl Default for PhotoRating {
    fn default() -> Self {
        PhotoRating::Maybe
    }
}

impl Display for PhotoRating {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhotoRating::Yes => f.write_str("Yes"),
            PhotoRating::Maybe => f.write_str("Maybe"),
            PhotoRating::No => f.write_str("No"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Photo {
    pub path: PathBuf,
    pub metadata: PhotoMetadata,
    pub thumbnail_hash: String,
}

impl Photo {
    pub fn new(path: PathBuf) -> Result<Self, PhotoError> {
        let metadata = PhotoMetadata::from_path(&path)?;
        let thumbnail_hash = hash64(&path.to_string_lossy()).to_string();
        Ok(Self {
            path,
            metadata,
            thumbnail_hash,
        })
    }

    pub async fn new_async(path: PathBuf) -> Result<Self, PhotoError> {
        let metadata = PhotoMetadata::from_path_async(&path).await?;
        let thumbnail_hash = hash64(&path.to_string_lossy()).to_string();
        Ok(Self {
            path,
            metadata,
            thumbnail_hash,
        })
    }

    pub fn file_name(&self) -> &str {
        match self.path.file_name() {
            Some(file_name) => file_name.to_str().unwrap_or("Unknown"),
            None => "Unknown",
        }
    }

    pub fn string_path(&self) -> String {
        self.path.display().to_string()
    }

    pub fn uri(&self) -> String {
        format!("file://{}", self.string_path())
    }

    pub fn thumbnail_uri(&self) -> String {
        format!("file://{}", self.thumbnail_path().unwrap().display())
    }

    pub fn thumbnail_path(&self) -> anyhow::Result<PathBuf> {
        let path = Dirs::Thumbnails
            .path()
            .join(&self.thumbnail_hash)
            .with_extension(self.path.extension().unwrap_or_default());
        Ok(path)
    }

    pub fn max_dimension(&self) -> MaxPhotoDimension {
        if self.metadata.rotated_width() >= self.metadata.rotated_height() {
            MaxPhotoDimension::Width
        } else {
            MaxPhotoDimension::Height
        }
    }

    pub fn aspect_ratio(&self) -> f32 {
        self.metadata.rotated_width() as f32 / self.metadata.rotated_height() as f32
    }

    pub fn size_with_max_size(&self, max_size: f32) -> (f32, f32) {
        let (width, height) = match self.max_dimension() {
            MaxPhotoDimension::Width => {
                let width = max_size;
                let height = width / self.aspect_ratio();
                (width, height)
            }
            MaxPhotoDimension::Height => {
                let height = max_size;
                let width = height * self.aspect_ratio();
                (width, height)
            }
        };
        (width, height)
    }

    pub fn rating(&self) -> PhotoRating {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        photo_manager.with_lock(|pm| pm.get_photo_rating(&self.path))
    }

    pub fn set_rating(&self, rating: PhotoRating) {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        photo_manager.with_lock_mut(|pm| pm.set_photo_rating(&self.path, rating));
    }

    pub fn tags(&self) -> HashSet<String> {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        photo_manager.with_lock(|pm| pm.get_photo_tags(&self.path))
    }

    pub fn set_tags(&self, tags: HashSet<String>) {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        photo_manager.with_lock_mut(|pm| pm.set_photo_tags(&self.path, tags));
    }

    pub fn add_tag(&self, tag: String) {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        photo_manager.with_lock_mut(|pm| pm.add_photo_tag(&self.path, tag));
    }

    pub fn remove_tag(&self, tag: &String) {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        photo_manager.with_lock_mut(|pm| pm.remove_photo_tag(&self.path, tag));
    }
}

impl PartialEq for Photo {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for Photo {}

impl Hash for Photo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

pub struct SaveOnDropPhoto<'a> {
    pub photo: &'a mut Photo,
}

impl<'a> SaveOnDropPhoto<'a> {
    pub fn new(photo: &'a mut Photo) -> Self {
        Self { photo }
    }
}

impl<'a> Deref for SaveOnDropPhoto<'a> {
    type Target = Photo;

    fn deref(&self) -> &Self::Target {
        self.photo
    }
}

impl DerefMut for SaveOnDropPhoto<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.photo
    }
}

impl<'a> Drop for SaveOnDropPhoto<'a> {
    fn drop(&mut self) {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        photo_manager.with_lock_mut(|photo_manager| {
            photo_manager.update_photo(self.photo.clone());
        });
    }
}
