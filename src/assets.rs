use eframe::egui::{include_image, ImageSource};

macro_rules! image_asset {
    ($name:ident, $path:expr_2021) => {
        pub fn $name() -> ImageSource<'static> {
            include_image!($path)
        }
    };
}

pub struct Asset;

impl Asset {
    image_asset!(resize, "assets/resize.png");
    image_asset!(rotate, "assets/rotate.png");
    image_asset!(larger, "assets/larger.png");
    image_asset!(smaller, "assets/smaller.png");
    image_asset!(add_page, "assets/add_page.png");
    image_asset!(horizontal_align_left, "assets/horizontal_align_left.png");
    image_asset!(
        horizontal_align_center,
        "assets/horizontal_align_center.png"
    );
    image_asset!(horizontal_align_right, "assets/horizontal_align_right.png");
    image_asset!(vertical_align_top, "assets/vertical_align_top.png");
    image_asset!(vertical_align_center, "assets/vertical_align_center.png");
    image_asset!(vertical_align_bottom, "assets/vertical_align_bottom.png");
    image_asset!(distribute_horizontal, "assets/horizontal_distribute.png");
    image_asset!(distribute_vertical, "assets/vertical_distribute.png");
}
