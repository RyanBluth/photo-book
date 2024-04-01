use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(FromRow)]
pub struct PhotoMetadata {
    id: i32,
    width: usize,
    height: usize,
    rotation: usize,
    rotated_width: usize,
    rotated_height: usize,
    camera: String,
    date_time: DateTime<Utc>,
    iso: u32,
    shutter_speed: String,
    aperture: String,
    focal_length: String,
}

#[derive(FromRow)]
pub struct Photo {
    id: i32,
    file_path: String,
    import_date: DateTime<Utc>,
    last_checked: DateTime<Utc>,
    rating: Option<u8>,
    category_color: Option<String>,
    is_favorite: bool,
    metadata_id: Option<i32>,
}

#[derive(FromRow)]
pub struct PhotoDirectory {
    id: i32,
    path: String,
}
