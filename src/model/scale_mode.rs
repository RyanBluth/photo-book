use std::fmt::{Display, Formatter};

use strum_macros::EnumIter;

#[derive(Debug, PartialEq, EnumIter, Clone, Copy)]
pub enum ScaleMode {
    Fit,
    Fill,
    Stretch,
}

impl Display for ScaleMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ScaleMode::Fit => write!(f, "Fit"),
            ScaleMode::Fill => write!(f, "Fill"),
            ScaleMode::Stretch => write!(f, "Stretch"),
        }
    }
}
