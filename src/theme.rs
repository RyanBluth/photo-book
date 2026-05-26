pub mod color {
    use egui::Color32;

    pub const BLACK: Color32 = Color32::BLACK;
    pub const WHITE: Color32 = Color32::WHITE;
    pub const TRANSPARENT: Color32 = Color32::TRANSPARENT;

    pub const ACCENT: Color32 = Color32::from_rgb(0x00, 0x7f, 0xff);
    pub const ACCENT_MUTED: Color32 = Color32::from_rgb(38, 54, 68);
    pub const BLUE_SOFT: Color32 = Color32::from_rgb(100, 150, 200);
    pub const SUCCESS: Color32 = Color32::GREEN;
    pub const ERROR: Color32 = Color32::RED;

    pub const SURFACE_DARK: Color32 = Color32::from_gray(30);
    pub const SURFACE: Color32 = Color32::from_gray(40);
    pub const SURFACE_MUTED: Color32 = Color32::from_gray(50);
    pub const SURFACE_STRONG: Color32 = Color32::from_gray(100);
    pub const SURFACE_EMPHASIS: Color32 = Color32::from_gray(150);

    pub const OVERLAY: Color32 = Color32::from_black_alpha(128);
    pub const WHITE_OVERLAY: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 50);
    pub const SELECTION_RECT: Color32 =
        Color32::from_rgba_unmultiplied_const(0x00, 0x7f, 0xff, 128);
}
