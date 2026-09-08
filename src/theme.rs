pub mod style;

pub mod color {
    use egui::Color32;

    pub const BLACK: Color32 = Color32::BLACK;
    pub const WHITE: Color32 = Color32::WHITE;
    pub const TRANSPARENT: Color32 = Color32::TRANSPARENT;

    pub const ACCENT: Color32 = Color32::from_rgb(55, 119, 201);
    pub const ACCENT_MUTED: Color32 = Color32::from_rgb(35, 53, 74);
    pub const BLUE_SOFT: Color32 = Color32::from_rgb(112, 169, 235);
    pub const SUCCESS: Color32 = Color32::from_rgb(119, 195, 151);
    pub const ERROR: Color32 = Color32::from_rgb(240, 128, 128);
    pub const WARNING: Color32 = Color32::from_rgb(225, 180, 102);

    pub const SURFACE_XX_DARK: Color32 = Color32::from_gray(18);
    pub const SURFACE_X_DARK: Color32 = Color32::from_gray(25);
    pub const SURFACE_DARK: Color32 = Color32::from_gray(31);
    pub const SIDE_PANEL_BACKGROUND: Color32 = SURFACE_X_DARK;
    pub const TOOLBAR_BACKGROUND: Color32 = SURFACE_X_DARK;
    pub const SURFACE: Color32 = Color32::from_gray(39);
    pub const SURFACE_MUTED: Color32 = Color32::from_gray(49);
    pub const SURFACE_STRONG: Color32 = Color32::from_gray(78);
    pub const SURFACE_EMPHASIS: Color32 = Color32::from_gray(148);
    pub const CONTROL_TEXT: Color32 = Color32::from_gray(210);

    pub const BORDER: Color32 = Color32::from_gray(53);
    pub const TEXT_MUTED: Color32 = Color32::from_gray(155);
    pub const ACCENT_HOVER: Color32 = Color32::from_rgb(65, 130, 215);
    pub const ACCENT_PRESSED: Color32 = Color32::from_rgb(43, 99, 173);

    pub const ICON: Color32 = CONTROL_TEXT;
    pub const ICON_ACTIVE: Color32 = BLUE_SOFT;

    pub const OVERLAY: Color32 = Color32::from_black_alpha(128);
    pub const WHITE_OVERLAY: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 50);
    pub const SELECTION_RECT: Color32 = Color32::from_rgba_unmultiplied_const(55, 119, 201, 96);
}
