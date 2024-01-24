use eframe::{
    emath::Rot2,
    epaint::{Pos2, Rect, Vec2},
};

macro_rules! guard_let {
    ($x:ident, $y:expr) => {
        let $x = $y;
        if $x.is_none() {
            return;
        }
        let $x = $x.unwrap();
    };
}

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

impl<T> Truncate for T
where
    T: ToString + std::fmt::Display,
{
    fn truncate(&self, max_length: usize) -> String {
        let string = self.to_string();
        if string.len() > max_length {
            format!("{}…", &string[0..max_length])
        } else {
            string
        }
    }
}

pub trait RectExt {
    fn constrain_to(&self, rect: Rect) -> Rect;
    fn rotate_bb_around_center(&self, angle: f32) -> Rect;
    fn to_local_space(&self, parent: Rect) -> Rect;
    fn to_world_space(&self, parent: Rect) -> Rect;
}

impl RectExt for Rect {
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
            constrained =
                constrained.translate(Vec2::new(0.0, rect.bottom() - constrained.bottom()));
        }
        constrained
    }

    fn rotate_bb_around_center(&self, angle: f32) -> Rect {
        let center = self.center().to_vec2();
        let top_left = self.min.to_vec2() - center;
        let top_right = Pos2::new(self.max.x, self.min.y).to_vec2() - center;
        let bottom_left = Pos2::new(self.min.x, self.max.y).to_vec2() - center;
        let bottom_right = self.max.to_vec2() - center;

        let rotation = Rot2::from_angle(angle);
        let rotated_top_left = rotation * top_left;
        let rotated_top_right = rotation * top_right;
        let rotated_bottom_left = rotation * bottom_left;
        let rotated_bottom_right = rotation * bottom_right;

        let rotated_corners = [
            rotated_top_left + center,
            rotated_top_right + center,
            rotated_bottom_left + center,
            rotated_bottom_right + center,
        ];

        // Find the minimum and maximum points among the rotated corners
        let min_x = rotated_corners
            .iter()
            .map(|p| p.x)
            .fold(f32::INFINITY, f32::min);
        let max_x = rotated_corners
            .iter()
            .map(|p| p.x)
            .fold(f32::NEG_INFINITY, f32::max);
        let min_y = rotated_corners
            .iter()
            .map(|p| p.y)
            .fold(f32::INFINITY, f32::min);
        let max_y = rotated_corners
            .iter()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max);

        Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(max_x, max_y))
    }

    fn to_local_space(&self, parent: Rect) -> Rect {
        let mut local = *self;
        local.min -= parent.min.to_vec2();
        local.max -= parent.min.to_vec2();
        local
    }

    fn to_world_space(&self, parent: Rect) -> Rect {
        let mut world = *self;
        world.min += parent.min.to_vec2();
        world.max += parent.min.to_vec2();
        world
    }
}

pub trait Toggle {
    fn toggle(&mut self);
}

impl Toggle for bool {
    fn toggle(&mut self) {
        *self = !*self;
    }
}
