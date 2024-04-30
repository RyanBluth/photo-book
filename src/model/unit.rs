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
