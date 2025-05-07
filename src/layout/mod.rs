pub mod grid_layout;
pub mod stack_layout;


#[derive(Debug, Clone)]
pub struct LayoutItem {
    pub aspect_ratio: f32,
    pub id: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Margin {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Margin {
    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub fn none() -> Self {
        Self {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        }
    }
}