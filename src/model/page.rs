use super::{editable_value::EditableValue, unit::Unit};
use egui::Vec2;

#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    width: f32,
    height: f32,
    ppi: i32,
    unit: Unit,
}

impl Page {
    pub fn new(width: f32, height: f32, ppi: i32, unit: Unit) -> Self {
        Self {
            width,
            height,
            ppi,
            unit,
        }
    }

    pub fn with_size_inches(width: f32, height: f32) -> Self {
        let ppi = 300;
        let unit = Unit::Inches;

        Self {
            width,
            height,
            ppi,
            unit,
        }
    }

    fn a4() -> Self {
        let ppi = 300;
        let unit = Unit::Inches;

        Self {
            width: 8.27,
            height: 11.69,
            ppi,
            unit,
        }
    }

    pub fn size_pixels(&self) -> Vec2 {
        match self.unit {
            Unit::Pixels => Vec2::new(self.width, self.height),
            Unit::Inches => Vec2::new(self.width * self.ppi as f32, self.height * self.ppi as f32),
            Unit::Centimeters => Vec2::new(
                self.width * (self.ppi as f32 / 2.54),
                self.height * (self.ppi as f32 / 2.54),
            ),
        }
    }

    pub fn size_mm(&self) -> Vec2 {
        match self.unit {
            Unit::Pixels => Vec2::new(
                self.width / (self.ppi as f32 / 2.54),
                self.height / (self.ppi as f32 / 2.54),
            ),
            Unit::Inches => Vec2::new(self.width * 25.4, self.height * 25.4),
            Unit::Centimeters => Vec2::new(self.width * 10.0, self.height * 10.0),
        }
    }

    pub fn size(&self) -> Vec2 {
        Vec2::new(self.width, self.height)
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn height(&self) -> f32 {
        self.height
    }

    pub fn ppi(&self) -> i32 {
        self.ppi
    }

    pub fn unit(&self) -> Unit {
        self.unit
    }

    pub fn aspect_ratio(&self) -> f32 {
        self.width / self.height
    }

    pub fn set_size(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
    }

    pub fn set_unit(&mut self, unit: Unit) {
        let size_pixels = self.size_pixels();
        match unit {
            Unit::Pixels => {
                self.width = size_pixels.x;
                self.height = size_pixels.y;
            }
            Unit::Inches => {
                self.width = size_pixels.x / self.ppi as f32;
                self.height = size_pixels.y / self.ppi as f32;
            }
            Unit::Centimeters => {
                self.width = size_pixels.x / (self.ppi as f32 / 2.54);
                self.height = size_pixels.y / (self.ppi as f32 / 2.54);
            }
        }
        self.unit = unit;
    }

    pub fn set_ppi(&mut self, ppi: i32) {
        self.ppi = ppi;
    }

    pub fn is_landscape(&self) -> bool {
        self.width > self.height
    }
}

impl Default for Page {
    fn default() -> Self {
        Self::a4()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageEditState {
    pub width: EditableValue<f32>,
    pub height: EditableValue<f32>,
    pub ppi: EditableValue<i32>,
    pub unit: EditableValue<Unit>,
}
