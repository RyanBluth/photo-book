
use egui::{FontFamily, FontId, RichText};

const ALIGN_HORIZONTAL_LEFT: &'static str = "\u{e00d}";
const ALIGN_HORIZONTAL_RIGHT: &'static str = "\u{e010}";
const ALIGN_HORIZONTAL_CENTER: &'static str = "\u{e00f}";
const ALIGN_VERTICAL_CENTER: &'static str = "\u{e011}";
const ALIGN_VERTICAL_TOP: &'static str = "\u{e00c}";
const ALIGN_VERTICAL_BOTTOM: &'static str = "\u{e015}";
const DISTRIBUTE_VERTICAL: &'static str = "\u{e076}";
const DISTRIBUTE_HORIZONTAL: &'static str = "\u{e014}";


static ICON_FONT_MEDIUM: once_cell::sync::Lazy<FontId> = once_cell::sync::Lazy::new(|| {
    FontId::new(16.0, FontFamily::Name("Material Symbols Outlined".into()))
});

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Icon {
    AlignHorizontalLeft,
    AlignHorizontalCenter,
    AlignVerticalCenter,
    AlignHorizontalRight,
    AlignVerticalTop,
    AlignVerticalBottom,
    DistributeVertical,
    DistributeHorizontal,
}

impl Icon {
    pub fn str(&self) -> &'static str {
        match self {
            Icon::AlignHorizontalLeft => ALIGN_HORIZONTAL_LEFT,
            Icon::AlignHorizontalCenter => ALIGN_HORIZONTAL_CENTER,
            Icon::AlignVerticalCenter => ALIGN_VERTICAL_CENTER,
            Icon::AlignHorizontalRight => ALIGN_HORIZONTAL_RIGHT,
            Icon::AlignVerticalTop => ALIGN_VERTICAL_TOP,
            Icon::AlignVerticalBottom => ALIGN_VERTICAL_BOTTOM,
            Icon::DistributeVertical => DISTRIBUTE_VERTICAL,
            Icon::DistributeHorizontal => DISTRIBUTE_HORIZONTAL,
        }
    }

    pub fn rich_text(&self) -> RichText {
        RichText::new(self.str()).font(ICON_FONT_MEDIUM.clone())
    }
}