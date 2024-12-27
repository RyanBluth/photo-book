use std::{fmt::Display, str::FromStr};

use strum_macros::EnumIter;

#[derive(Debug, Clone, PartialEq, Copy, EnumIter)]
pub enum Unit {
    Pixels,
    Inches,
    Centimeters,
}

impl Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unit::Pixels => write!(f, "Pixels"),
            Unit::Inches => write!(f, "Inches"),
            Unit::Centimeters => write!(f, "Centimeters"),
        }
    }
}

impl FromStr for Unit {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Pixels" => Ok(Unit::Pixels),
            "Inches" => Ok(Unit::Inches),
            "Centimeters" => Ok(Unit::Centimeters),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, EnumIter)]
pub enum PageSizePreset {
    Custom,
    Square8x8,
    Landscape8x11,
    Portrait11x8,
    Square12x12,
}

impl PageSizePreset {
    pub fn dimensions(&self) -> Option<(f32, f32)> {
        match self {
            Self::Custom => None,
            Self::Square8x8 => Some((8.0, 8.0)),
            Self::Landscape8x11 => Some((11.0, 8.0)),
            Self::Portrait11x8 => Some((8.0, 11.0)),
            Self::Square12x12 => Some((12.0, 12.0)),
        }
    }
}

impl std::fmt::Display for PageSizePreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Custom => write!(f, "Custom"),
            Self::Square8x8 => write!(f, "8\" × 8\""),
            Self::Landscape8x11 => write!(f, "11\" × 8\""),
            Self::Portrait11x8 => write!(f, "8\" × 11\""),
            Self::Square12x12 => write!(f, "12\" × 12\""),
        }
    }
}
