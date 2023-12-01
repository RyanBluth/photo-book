use eframe::epaint::{Rect, Vec2};


pub fn partition_iterator<T>(iter: impl Iterator<Item = T>, partitions: usize) -> Vec<Vec<T>> {
    let mut output: Vec<Vec<T>> = (0..partitions).map(|_| Vec::new()).collect();
    for (i, item) in iter.enumerate() {
        let partition_index = i % partitions;
        output[partition_index].push(item);
    }
    output
}


pub trait Truncate {
    fn truncate(&self, max_length: usize) -> String;
}

impl<T> Truncate for T where T: ToString + std::fmt::Display {
    fn truncate(&self, max_length: usize) -> String {
        let string = self.to_string();
        if string.len() > max_length {
            format!("{}…", &string[0..max_length])
        } else {
            string
        }
    }
}

pub trait ConstrainRect {
    fn constrain_to(&self, rect: Rect) -> Rect;
}

impl ConstrainRect for Rect {
    fn constrain_to(&self, rect: Rect) -> Rect {
        let mut constrained = *self;
        if constrained.left() < rect.left() {
            constrained = constrained.translate(Vec2::new(rect.left() - constrained.left(), 0.0));
        }
        if constrained.right() > rect.right() {
            constrained = constrained.translate(Vec2::new(rect.right() - constrained.right(), 0.0));
        }
        if constrained.top() < rect.top() {
            constrained = constrained.translate(Vec2::new(0.0, rect.top() - constrained.top()));
        }
        if constrained.bottom() > rect.bottom() {
            constrained = constrained.translate(Vec2::new(0.0, rect.bottom() - constrained.bottom()));
        }
        constrained
    }
}