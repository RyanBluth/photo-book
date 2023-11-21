use std::{
    collections::HashMap, f32::consts::PI, fmt::Display, fs::File, hash::Hash, hash::Hasher,
    io::BufReader, path::PathBuf,
};

use crate::dependencies::SingletonFor;

use anyhow::anyhow;
use eframe::{
    emath::Rot2,
    epaint::{Pos2, Rect, Vec2},
};

use exif::{In, Reader, SRational, Tag, Value};

macro_rules! metadata_fields {
    ($(($name:ident, $type:ty)),*) => {
        #[derive(Debug, Clone)]
        pub enum PhotoMetadataField {
            $( $name($type), )*
        }

        #[derive(Debug, Clone, Eq, PartialEq, Hash, Copy)]
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
pub enum MaxPhotoDimension {
    Width,
    Height,
}

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone)]
pub struct MetadataCollection {
    fields: HashMap<PhotoMetadataFieldLabel, PhotoMetadataField>,
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
    (DateTime, String),
    (ISO, u32),
    (ShutterSpeed, SRational),
    (Aperture, SRational),
    (FocalLength, SRational)
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

#[derive(Debug, Clone)]
pub struct PhotoMetadata {
    pub fields: MetadataCollection,
}

impl PhotoMetadata {
    pub fn from_path(path: &PathBuf) -> Self {
        let mut fields = MetadataCollection::new();
        fields.insert(PhotoMetadataField::Path(path.clone()));

        let file = File::open(path).unwrap();
        let exif = Reader::new().read_from_container(&mut BufReader::new(&file));

        let size = imagesize::size(path.clone()).unwrap();
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
                match field.value {
                    Value::Ascii(ref vec) => {
                        if let Some(value) = vec.get(0) {
                            fields.insert(PhotoMetadataField::Camera(
                                String::from_utf8_lossy(value).to_string(),
                            ));
                        }
                    }
                    _ => {}
                }
            };
            if let Some(field) = exif.get_field(Tag::DateTimeOriginal, In::PRIMARY) {
                match field.value {
                    Value::Ascii(ref vec) => {
                        if let Some(value) = vec.get(0) {
                            fields.insert(PhotoMetadataField::DateTime(
                                String::from_utf8_lossy(value).to_string(),
                            ));
                        }
                    }
                    _ => {}
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
                        if let Some(value) = vec.get(0) {
                            fields.insert(PhotoMetadataField::ShutterSpeed(SRational {
                                num: value.num as i32,
                                denom: value.denom as i32,
                            }));
                        }
                    }
                    Value::SRational(ref vec) => {
                        if let Some(value) = vec.get(0) {
                            fields.insert(PhotoMetadataField::ShutterSpeed(*value));
                        }
                    }
                    _ => {}
                }
            }

            if let Some(field) = exif.get_field(Tag::FNumber, In::PRIMARY) {
                match field.value {
                    Value::Rational(ref vec) => {
                        if let Some(value) = vec.get(0) {
                            fields.insert(PhotoMetadataField::Aperture(SRational {
                                num: value.num as i32,
                                denom: value.denom as i32,
                            }));
                        }
                    }
                    _ => {}
                }
            }

            if let Some(field) = exif.get_field(Tag::FocalLength, In::PRIMARY) {
                match field.value {
                    Value::Rational(ref vec) => {
                        if let Some(value) = vec.get(0) {
                            fields.insert(PhotoMetadataField::FocalLength(SRational {
                                num: value.num as i32,
                                denom: value.denom as i32,
                            }));
                        }
                    }
                    _ => {}
                }
            }
        } else {
            fields.insert(PhotoMetadataField::Rotation(PhotoRotation::Normal));
            fields.insert(PhotoMetadataField::RotatedWidth(width));
            fields.insert(PhotoMetadataField::RotatedHeight(height));
        }

        Self { fields }
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

#[derive(Debug, Clone)]
pub struct Photo {
    pub path: PathBuf,
    pub metadata: PhotoMetadata,
}

impl Photo {
    pub fn new(path: PathBuf) -> Self {
        let metadata = PhotoMetadata::from_path(&path);

        Self { path, metadata }
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
        let mut path = self.path.clone();
        let file_name = path
            .file_name()
            .ok_or(anyhow!("Failed to get file name"))?
            .to_str()
            .ok_or(anyhow!("Failed to convert file name to str"))?
            .to_string();
        path.pop();
        path.push(".thumb");
        path.push(file_name);
        Ok(path)
    }

    pub fn max_dimension(&self) -> MaxPhotoDimension {
        if self.metadata.width() >= self.metadata.height() {
            MaxPhotoDimension::Width
        } else {
            MaxPhotoDimension::Height
        }
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
