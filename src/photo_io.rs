use std::{
    cmp::Reverse,
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context as _, anyhow, bail};
use chrono::{DateTime, Utc};
use fxhash::hash64;
use image::{DynamicImage, ExtendedColorType, ImageEncoder, RgbImage, codecs::jpeg::JpegEncoder};
use rsraw::{BIT_DEPTH_8, ImageFormat as RawImageFormat, RawImage, ThumbFormat, ThumbnailImage};
use tokio::task::spawn_blocking;

use crate::dirs::Dirs;

pub const STANDARD_PHOTO_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png"];
pub const RAW_PHOTO_EXTENSIONS: &[&str] = &[
    "3fr", "arw", "cap", "cr2", "cr3", "crw", "dcr", "dng", "erf", "fff", "gpr", "iiq", "kdc",
    "mef", "mos", "mrw", "nef", "nrw", "orf", "pef", "ptx", "raf", "raw", "rw2", "rwl", "sr2",
    "srf", "srw", "x3f",
];
pub const SUPPORTED_PHOTO_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "3fr", "arw", "cap", "cr2", "cr3", "crw", "dcr", "dng", "erf", "fff",
    "gpr", "iiq", "kdc", "mef", "mos", "mrw", "nef", "nrw", "orf", "pef", "ptx", "raf", "raw",
    "rw2", "rwl", "sr2", "srf", "srw", "x3f",
];

#[derive(Debug, Clone)]
pub struct RawPhotoMetadata {
    pub width: u32,
    pub height: u32,
    pub camera: Option<String>,
    pub date_time: Option<DateTime<Utc>>,
    pub iso: Option<u32>,
    pub shutter_speed: Option<f32>,
    pub aperture: Option<f32>,
    pub focal_length: Option<f32>,
}

pub fn extension_lowercase(path: &Path) -> Option<String> {
    path.extension()?
        .to_str()
        .map(|ext| ext.to_ascii_lowercase())
}

pub fn is_standard_photo_path(path: &Path) -> bool {
    extension_lowercase(path)
        .as_deref()
        .is_some_and(|ext| STANDARD_PHOTO_EXTENSIONS.contains(&ext))
}

pub fn is_raw_photo_path(path: &Path) -> bool {
    extension_lowercase(path)
        .as_deref()
        .is_some_and(|ext| RAW_PHOTO_EXTENSIONS.contains(&ext))
}

pub fn is_supported_photo_path(path: &Path) -> bool {
    is_standard_photo_path(path) || is_raw_photo_path(path)
}

pub fn thumbnail_extension(path: &Path) -> String {
    if is_raw_photo_path(path) {
        "jpg".to_string()
    } else {
        extension_lowercase(path).unwrap_or_else(|| "jpg".to_string())
    }
}

pub fn rendered_photo_path(source_path: &Path) -> PathBuf {
    let hash = hash64(&source_path.to_string_lossy()).to_string();
    Dirs::RenderedPhotos.path().join(hash).with_extension("jpg")
}

pub fn file_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

pub fn raw_metadata_from_path(path: &Path) -> anyhow::Result<RawPhotoMetadata> {
    let bytes =
        std::fs::read(path).with_context(|| format!("read RAW metadata {}", path.display()))?;
    let raw = RawImage::open(&bytes)?;
    let info = raw.full_info();
    let camera = first_non_empty([info.normalized_model, info.model]);

    Ok(RawPhotoMetadata {
        width: info.width,
        height: info.height,
        camera,
        date_time: info.datetime.map(|date_time| date_time.with_timezone(&Utc)),
        iso: (info.iso_speed > 0).then_some(info.iso_speed),
        shutter_speed: positive(info.shutter),
        aperture: positive(info.aperture),
        focal_length: positive(info.focal_len),
    })
}

pub async fn ensure_rendered_photo(source_path: PathBuf) -> anyhow::Result<PathBuf> {
    spawn_blocking(move || ensure_rendered_photo_blocking(&source_path)).await?
}

pub fn ensure_rendered_photo_blocking(source_path: &Path) -> anyhow::Result<PathBuf> {
    if !is_raw_photo_path(source_path) {
        return Ok(source_path.to_path_buf());
    }

    let rendered_path = rendered_photo_path(source_path);
    if rendered_path.exists() {
        return Ok(rendered_path);
    }

    if let Some(parent) = rendered_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create RAW render cache {}", parent.display()))?;
    }

    let image = decode_raw_image(source_path, false)?;
    write_jpeg_atomically(&rendered_path, &image, 92)?;

    Ok(rendered_path)
}

pub fn decode_raw_image(source_path: &Path, half_size: bool) -> anyhow::Result<DynamicImage> {
    let bytes = std::fs::read(source_path)
        .with_context(|| format!("read RAW image {}", source_path.display()))?;
    decode_raw_image_from_bytes(source_path, &bytes, half_size)
}

pub fn decode_raw_thumbnail_image(source_path: &Path) -> anyhow::Result<DynamicImage> {
    let bytes = std::fs::read(source_path)
        .with_context(|| format!("read RAW image {}", source_path.display()))?;

    let mut raw = RawImage::open(&bytes)?;
    if let Ok(mut thumbnails) = raw.extract_thumbs() {
        thumbnails.sort_by_key(|thumbnail| Reverse(thumbnail_area(thumbnail)));
        for thumbnail in thumbnails {
            if let Ok(image) = decode_raw_thumbnail(thumbnail) {
                return Ok(image);
            }
        }
    }

    drop(raw);
    decode_raw_image_from_bytes(source_path, &bytes, true)
}

fn decode_raw_image_from_bytes(
    source_path: &Path,
    bytes: &[u8],
    half_size: bool,
) -> anyhow::Result<DynamicImage> {
    let mut raw = RawImage::open(&bytes)?;
    raw.set_use_camera_wb(true);
    raw.set_use_camera_matrix(true);
    raw.set_half_size(half_size);
    // Keep cache files in source orientation; the UI already applies EXIF rotation.
    raw.as_mut().params.user_flip = 0;
    raw.unpack()?;

    let processed = raw.process::<BIT_DEPTH_8>()?;
    match processed.image_format() {
        RawImageFormat::Jpeg => image::load_from_memory(&processed)
            .with_context(|| format!("decode processed RAW JPEG {}", source_path.display())),
        RawImageFormat::Bitmap => bitmap_to_dynamic_image(
            processed.width(),
            processed.height(),
            processed.colors() as usize,
            processed.to_vec(),
        ),
    }
}

fn decode_raw_thumbnail(thumbnail: ThumbnailImage) -> anyhow::Result<DynamicImage> {
    match thumbnail.format {
        ThumbFormat::Jpeg => {
            image::load_from_memory(&thumbnail.data).context("decode embedded RAW JPEG thumbnail")
        }
        ThumbFormat::Bitmap => bitmap_to_dynamic_image(
            thumbnail.width,
            thumbnail.height,
            thumbnail.colors as usize,
            thumbnail.data,
        ),
        format => bail!("unsupported embedded RAW thumbnail format: {:?}", format),
    }
}

fn bitmap_to_dynamic_image(
    width: u32,
    height: u32,
    colors: usize,
    data: Vec<u8>,
) -> anyhow::Result<DynamicImage> {
    if colors < 3 {
        bail!("processed RAW image has {} color channel(s)", colors);
    }

    let rgb = if colors == 3 {
        data
    } else {
        data.chunks_exact(colors)
            .flat_map(|pixel| pixel[0..3].iter().copied())
            .collect()
    };

    let image = RgbImage::from_raw(width, height, rgb)
        .ok_or_else(|| anyhow!("processed RAW image buffer has invalid dimensions"))?;
    Ok(DynamicImage::ImageRgb8(image))
}

fn thumbnail_area(thumbnail: &ThumbnailImage) -> u64 {
    u64::from(thumbnail.width) * u64::from(thumbnail.height)
}

fn write_jpeg_atomically(path: &Path, image: &DynamicImage, quality: u8) -> anyhow::Result<()> {
    let tmp_path = path.with_extension("jpg.tmp");
    let rgb = image.to_rgb8();

    {
        let file = File::create(&tmp_path)
            .with_context(|| format!("create temporary RAW render {}", tmp_path.display()))?;
        let mut writer = BufWriter::new(file);
        JpegEncoder::new_with_quality(&mut writer, quality).write_image(
            &rgb,
            rgb.width(),
            rgb.height(),
            ExtendedColorType::Rgb8,
        )?;
        writer.flush()?;
    }

    std::fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "move temporary RAW render {} to {}",
            tmp_path.display(),
            path.display()
        )
    })?;

    Ok(())
}

fn first_non_empty(values: impl IntoIterator<Item = String>) -> Option<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

fn positive(value: f32) -> Option<f32> {
    value
        .is_finite()
        .then_some(value)
        .filter(|value| *value > 0.0)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn supported_extensions_are_case_insensitive() {
        assert!(is_standard_photo_path(Path::new("photo.JPG")));
        assert!(is_standard_photo_path(Path::new("photo.PnG")));
        assert!(is_raw_photo_path(Path::new("photo.CR2")));
        assert!(is_raw_photo_path(Path::new("photo.ArW")));
        assert!(is_supported_photo_path(Path::new("photo.NEF")));
        assert!(!is_supported_photo_path(Path::new("photo.txt")));
    }

    #[test]
    fn raw_thumbnails_use_jpeg_extension() {
        assert_eq!(thumbnail_extension(Path::new("photo.CR3")), "jpg");
        assert_eq!(thumbnail_extension(Path::new("photo.jpeg")), "jpeg");
    }
}
