use egui::Vec2;

use super::{editable_value::EditableValue, unit::Unit};


#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    size: Vec2,
    ppi: i32,
    unit: Unit,
}

impl Page {

    pub fn with_size_inches(size: Vec2) -> Self {
        let ppi = 300;
        let unit = Unit::Inches;

        Self {
            size,
            ppi,
            unit,
        }
    }

    fn a4() -> Self {
        let ppi = 300;
        let unit = Unit::Inches;

        Self {
            size: Vec2::new(8.27, 11.69),
            ppi,
            unit,
        }
    }

    pub fn size_pixels(&self) -> Vec2 {
        match self.unit {
            Unit::Pixels => self.size,
            Unit::Inches => self.size * self.ppi as f32,
            Unit::Centimeters => self.size * (self.ppi as f32 / 2.54),
        }
    }

    pub fn size(&self) -> Vec2 {
        self.size
    }

    pub fn ppi(&self) -> i32 {
        self.ppi
    }

    pub fn unit(&self) -> Unit {
        self.unit
    }

    pub fn aspect_ratio(&self) -> f32 {
        self.size.x / self.size.y
    }

    pub fn set_size(&mut self, size: Vec2) {
        self.size = size;
    }

    pub fn set_unit(&mut self, unit: Unit) {
        let size_pixels = self.size_pixels();
        match unit {
            Unit::Pixels => self.size = size_pixels,
            Unit::Inches => self.size = size_pixels / self.ppi as f32,
            Unit::Centimeters => self.size = size_pixels / (self.ppi as f32 / 2.54),
        }
        self.unit = unit;
    }

    pub fn set_ppi(&mut self, ppi: i32) {
        self.ppi = ppi;
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