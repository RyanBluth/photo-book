use std::{borrow::BorrowMut, fs::File, io::BufReader, path::PathBuf, f32::consts::PI};

use crate::{
    dependencies::{Dependency, SingletonFor},
    image_cache::ImageCache,
};

use anyhow::anyhow;
use eframe::{
    egui::{self, load::SizedTexture, Context, SizeHint, TextureOptions},
    epaint::{util::FloatOrd, Vec2},
};
use log::error;

use exif::{DateTime, In, Reader, Tag, Value};

pub enum MaxPhotoDimension {
    Width(f32),
    Height(f32),
}

#[derive(Debug, Clone, Copy)]
pub enum PhotoRotation {
    Normal,
    MirrorHorizontal,
    Rotate180,
    MirrorVertical,
    MirrorHorizontalAndRotate270CW,
    Rotate90CW,
    MirrorHorizontalAndRotate90CW,
    Rotate270CW,
}

impl PhotoRotation {

    pub fn radians(&self) -> f32 {
        match self {
           Self::Normal => 0.0,
           Self::MirrorHorizontal => todo!(),
           Self::Rotate180 => todo!(),
           Self::MirrorVertical => todo!(),
           Self::MirrorHorizontalAndRotate270CW => todo!(),
           Self::Rotate90CW => PI / 2.0,
           Self::MirrorHorizontalAndRotate90CW => todo!(),
           Self::Rotate270CW => (3.0 * PI) / 2.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PhotoMetadata {
    pub width: f32,
    pub height: f32,
    pub rotation: PhotoRotation,
}

impl PhotoMetadata {
    pub fn from_path(path: &PathBuf) -> Self {
        let file = File::open(&path).unwrap();
        let exif = Reader::new()
            .read_from_container(&mut BufReader::new(&file))
            .unwrap();

        let mut width: Option<u32> = None;
        if let Some(field) = exif.get_field(Tag::PixelXDimension, In::PRIMARY) {
            if let Some(value) = field.value.get_uint(0) {
                width = Some(value);
            }
        }

        let mut height: Option<u32> = None;
        if let Some(field) = exif.get_field(Tag::PixelYDimension, In::PRIMARY) {
            if let Some(value) = field.value.get_uint(0) {
                height = Some(value);
            }
        }

        let mut rotation = PhotoRotation::Normal;
        if let Some(field) = exif.get_field(Tag::Orientation, In::PRIMARY) {
            if let (Some(value), Some(width_val), Some(height_val)) =
                (field.value.get_uint(0), width, height)
            {
                match value {
                    1 => {
                        // Normal
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
                        rotation = PhotoRotation::MirrorVertical;
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
        }

        match (width, height) {
            (Some(width), Some(height)) => Self {
                width: width as f32,
                height: height as f32,
                rotation,
            },
            _ => {
                let size = imagesize::size(path.clone()).unwrap();
                Self {
                    width: size.width as f32,
                    height: size.height as f32,
                    rotation,
                }
            }
        }
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
        if self.metadata.width > self.metadata.height {
            MaxPhotoDimension::Width(self.metadata.width)
        } else {
            MaxPhotoDimension::Height(self.metadata.height)
        }
    }
}
