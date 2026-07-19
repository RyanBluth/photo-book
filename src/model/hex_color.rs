use std::{fmt::Display, str::FromStr};

use egui::{
    Color32,
    ecolor::{HexColor as EguiHexColor, ParseHexColorError},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HexColor(pub Color32);

impl Display for HexColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        EguiHexColor::Hex8(self.0).fmt(f)
    }
}

impl FromStr for HexColor {
    type Err = ParseHexColorError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        let hex = value.strip_prefix('#').unwrap_or(value);
        let parsed = if value.starts_with('#') {
            EguiHexColor::from_str(value)?
        } else {
            EguiHexColor::from_str_without_hash(value)?
        };

        // egui encodes additive colors as non-zero RGB with zero alpha. CSS
        // hex cannot distinguish that from a transparent color, so preserve
        // egui's representation when the RGB channels are non-zero.
        let rgb_hex = match hex.len() {
            4 if hex.ends_with('0') && &hex[..3] != "000" => Some(&hex[..3]),
            8 if hex.ends_with("00") && &hex[..6] != "000000" => Some(&hex[..6]),
            _ => None,
        };
        let color = if let Some(rgb_hex) = rgb_hex {
            let rgb = EguiHexColor::from_str_without_hash(rgb_hex)?.color();
            let [red, green, blue, _] = rgb.to_srgba_unmultiplied();
            Color32::from_rgba_premultiplied(red, green, blue, 0)
        } else {
            parsed.color()
        };

        Ok(Self(color))
    }
}

#[cfg(test)]
mod tests {
    use super::HexColor;
    use egui::Color32;

    #[test]
    fn accepts_rgb_and_rgba_values() {
        assert_eq!(
            "#102030".parse::<HexColor>().unwrap(),
            HexColor(Color32::from_rgb(0x10, 0x20, 0x30))
        );
        assert_eq!(
            "10203040".parse::<HexColor>().unwrap(),
            HexColor(Color32::from_rgba_unmultiplied(0x10, 0x20, 0x30, 0x40))
        );
        assert_eq!(
            "10203040".parse::<HexColor>().unwrap().to_string(),
            "#10203040"
        );
    }

    #[test]
    fn additive_colors_round_trip() {
        let additive = HexColor(Color32::from_rgba_premultiplied(0x10, 0x20, 0x30, 0));
        assert_eq!(additive.to_string().parse::<HexColor>().unwrap(), additive);
    }
}
