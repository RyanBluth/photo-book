use std::path::Path;

use image::{DynamicImage, ImageDecoder, ImageReader};

pub(crate) fn decode_oriented_image(
    path: &Path,
    max_texture_side: u32,
) -> Result<DynamicImage, String> {
    let file_bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let format = image::guess_format(&file_bytes).map_err(|error| error.to_string())?;
    let reader = ImageReader::with_format(std::io::Cursor::new(file_bytes), format);
    let mut decoder = reader.into_decoder().map_err(|error| error.to_string())?;
    let orientation = decoder.orientation().map_err(|error| error.to_string())?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|error| error.to_string())?;
    image.apply_orientation(orientation);

    if max_texture_side > 0
        && (image.width() > max_texture_side || image.height() > max_texture_side)
    {
        image = image.resize(
            max_texture_side,
            max_texture_side,
            image::imageops::FilterType::Triangle,
        );
    }

    Ok(image)
}

pub(crate) fn path_version_key(path: &Path) -> String {
    let Ok(metadata) = path.metadata() else {
        return "missing".to_owned();
    };

    let modified = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    format!("{}:{}", metadata.len(), modified)
}

#[cfg(test)]
mod tests {
    use image::{Rgb, RgbImage, codecs::jpeg::JpegEncoder};

    use super::*;

    #[test]
    fn decode_applies_exif_orientation() {
        let source = RgbImage::from_pixel(3, 2, Rgb([128, 64, 32]));
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, 90)
            .encode_image(&source)
            .unwrap();

        // EXIF orientation 6 rotates the decoded 3x2 image 90 degrees clockwise.
        let exif_orientation_6 = [
            0xff, 0xe1, 0x00, 0x22, b'E', b'x', b'i', b'f', 0x00, 0x00, b'I', b'I', 0x2a, 0x00,
            0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        jpeg.splice(2..2, exif_orientation_6);

        let path = std::env::temp_dir().join(format!(
            "photo_book_oriented_decode_{}_{}.jpg",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, jpeg).unwrap();

        let decoded = decode_oriented_image(&path, 100).unwrap();
        let _ = std::fs::remove_file(path);

        assert_eq!((decoded.width(), decoded.height()), (2, 3));
    }
}
