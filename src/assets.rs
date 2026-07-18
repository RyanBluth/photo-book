use eframe::egui::{ImageSource, include_image};

macro_rules! image_asset {
    ($name:ident, $path:expr_2021) => {
        pub fn $name() -> ImageSource<'static> {
            include_image!($path)
        }
    };
}

macro_rules! icon {
    ($name:ident, $path:literal) => {
        pub fn $name() -> ImageSource<'static> {
            include_image!(concat!("assets/icons/", $path))
        }
    };
}

pub struct Asset {}

impl Asset {
    icon!(resize, "resize.png");
    icon!(rotate, "rotate.png");
    icon!(larger, "larger.png");
    icon!(smaller, "smaller.png");
    icon!(add_page, "add_page.png");
    icon!(horizontal_align_left, "horizontal_align_left.png");
    icon!(horizontal_align_center, "horizontal_align_center.png");
    icon!(horizontal_align_right, "horizontal_align_right.png");
    icon!(vertical_align_top, "vertical_align_top.png");
    icon!(vertical_align_center, "vertical_align_center.png");
    icon!(vertical_align_bottom, "vertical_align_bottom.png");
    icon!(distribute_horizontal, "horizontal_distribute.png");
    icon!(distribute_vertical, "vertical_distribute.png");

    // Toolbar icons
    icon!(icon_select, "select.svg");
    icon!(icon_text, "text.svg");
    icon!(icon_rectangle, "rectangle.svg");
    icon!(icon_ellipse, "ellipse.svg");
    icon!(icon_line, "line.svg");

    // Status Bar Icons
    icon!(sidebar_right, "sidebar-right.svg");
    icon!(sidebar_left, "sidebar-left.svg");
    icon!(logs, "logs.svg");
}
