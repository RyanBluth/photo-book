use eframe::egui::{include_image, ImageSource};

macro_rules! image_asset {
    ($name:ident, $path:expr) => {
        pub fn $name() -> ImageSource<'static> {
            include_image!($path)
        }
    };
}

pub struct Asset;

impl Asset {
    image_asset!(resize, "assets/resize.png");
    image_asset!(rotate, "assets/rotate.png");
}
