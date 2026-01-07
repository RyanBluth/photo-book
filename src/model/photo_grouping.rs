use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PhotoGrouping {
    Rating,
    Tag,
    Date,
}

impl Default for PhotoGrouping {
    fn default() -> Self {
        Self::Date
    }
}
