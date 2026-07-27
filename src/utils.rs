use std::{fmt::Display, str::FromStr};

use chrono::{ParseError, TimeZone, Utc};
use eframe::{
    emath::Rot2,
    epaint::{Mesh, Pos2, Rect, Vec2, Vertex},
};
use egui::{Align, Id, InnerResponse, Layout, Response, Sense, TextBuffer, TextEdit, Ui};

use crate::{
    cursor_manager::CursorManager, dep_mut, model::editable_value::EditableValue,
    sizing_manager::SizingManager, theme::style,
};

pub fn partition_iterator<T>(iter: impl Iterator<Item = T>, partitions: usize) -> Vec<Vec<T>> {
    let mut output: Vec<Vec<T>> = (0..partitions).map(|_| Vec::new()).collect();
    for (i, item) in iter.enumerate() {
        let partition_index = i % partitions;
        output[partition_index].push(item);
    }
    output
}

pub trait MeshExt {
    /// Clip all mesh triangles to `rect`, interpolating vertex UVs and colors
    /// where the clipping boundary creates new vertices.
    fn clip_mesh(&self, rect: Rect) -> Mesh;
}

impl MeshExt for Mesh {
    fn clip_mesh(&self, rect: Rect) -> Mesh {
        let empty_mesh = || Mesh::with_texture(self.texture_id);
        if self.is_empty() || !rect.is_positive() {
            return empty_mesh();
        }

        let mesh_bounds = self.calc_bounds();
        if rect.contains_rect(mesh_bounds) {
            return self.clone();
        }
        if !rect.intersects(mesh_bounds) {
            return empty_mesh();
        }

        let mut clipped_mesh = empty_mesh();

        for [a, b, c] in self.triangles() {
            let triangle = [
                self.vertices[a as usize],
                self.vertices[b as usize],
                self.vertices[c as usize],
            ];
            let triangle_bounds = Rect::from_points(&triangle.map(|vertex| vertex.pos));

            if rect.contains_rect(triangle_bounds) {
                append_triangle(&mut clipped_mesh, triangle);
                continue;
            }
            if !rect.intersects(triangle_bounds) {
                continue;
            }

            let mut polygon = triangle.to_vec();

            polygon = clip_polygon_to_edge(
                polygon,
                |vertex| vertex.pos.x >= rect.left(),
                |start, end| vertex_at_x(start, end, rect.left()),
            );
            polygon = clip_polygon_to_edge(
                polygon,
                |vertex| vertex.pos.x <= rect.right(),
                |start, end| vertex_at_x(start, end, rect.right()),
            );
            polygon = clip_polygon_to_edge(
                polygon,
                |vertex| vertex.pos.y >= rect.top(),
                |start, end| vertex_at_y(start, end, rect.top()),
            );
            polygon = clip_polygon_to_edge(
                polygon,
                |vertex| vertex.pos.y <= rect.bottom(),
                |start, end| vertex_at_y(start, end, rect.bottom()),
            );
            deduplicate_polygon(&mut polygon);

            if polygon.len() < 3 {
                continue;
            }

            let first_index = clipped_mesh.vertices.len() as u32;
            let mut added_vertices = false;
            for index in 1..polygon.len() - 1 {
                if is_degenerate_triangle(polygon[0], polygon[index], polygon[index + 1]) {
                    continue;
                }
                if !added_vertices {
                    clipped_mesh.vertices.extend(polygon.iter().copied());
                    added_vertices = true;
                }
                clipped_mesh.indices.extend_from_slice(&[
                    first_index,
                    first_index + index as u32,
                    first_index + index as u32 + 1,
                ]);
            }
        }

        clipped_mesh
    }
}

fn clip_polygon_to_edge(
    vertices: Vec<Vertex>,
    is_inside: impl Fn(Vertex) -> bool,
    intersection: impl Fn(Vertex, Vertex) -> Vertex,
) -> Vec<Vertex> {
    let Some(mut previous) = vertices.last().copied() else {
        return Vec::new();
    };
    let mut previous_inside = is_inside(previous);
    let mut clipped = Vec::with_capacity(vertices.len() + 1);

    for current in vertices {
        let current_inside = is_inside(current);
        match (previous_inside, current_inside) {
            (true, true) => clipped.push(current),
            (true, false) => clipped.push(intersection(previous, current)),
            (false, true) => {
                clipped.push(intersection(previous, current));
                clipped.push(current);
            }
            (false, false) => {}
        }
        previous = current;
        previous_inside = current_inside;
    }

    clipped
}

fn deduplicate_polygon(vertices: &mut Vec<Vertex>) {
    vertices.dedup();
    if vertices.len() > 1 && vertices.first() == vertices.last() {
        vertices.pop();
    }
}

fn append_triangle(mesh: &mut Mesh, vertices: [Vertex; 3]) {
    if is_degenerate_triangle(vertices[0], vertices[1], vertices[2]) {
        return;
    }

    let first_index = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(&vertices);
    mesh.indices
        .extend_from_slice(&[first_index, first_index + 1, first_index + 2]);
}

fn is_degenerate_triangle(a: Vertex, b: Vertex, c: Vertex) -> bool {
    let ab = b.pos - a.pos;
    let ac = c.pos - a.pos;
    (ab.x * ac.y - ab.y * ac.x).abs() <= f32::EPSILON
}

fn vertex_at_x(start: Vertex, end: Vertex, x: f32) -> Vertex {
    let t = (x - start.pos.x) / (end.pos.x - start.pos.x);
    interpolate_vertex(start, end, t)
}

fn vertex_at_y(start: Vertex, end: Vertex, y: f32) -> Vertex {
    let t = (y - start.pos.y) / (end.pos.y - start.pos.y);
    interpolate_vertex(start, end, t)
}

fn interpolate_vertex(start: Vertex, end: Vertex, t: f32) -> Vertex {
    let t = t.clamp(0.0, 1.0);

    Vertex {
        pos: start.pos + (end.pos - start.pos) * t,
        uv: start.uv + (end.uv - start.uv) * t,
        color: start.color.lerp_to_gamma(end.color, t),
    }
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
    fn rotate_bb_around_point(&self, angle: f32, point: Pos2) -> Rect;
    fn _constrain_to(&self, rect: Rect) -> Rect;
    fn rotate_bb_around_center(&self, angle: f32) -> Rect;
    fn to_local_space(&self, parent: Rect) -> Rect;
    fn to_world_space(&self, parent: Rect) -> Rect;
    fn _scale(&self, scale: f32) -> Rect;
    fn translate_left_to(&self, new_left: f32) -> Rect;
    fn translate_right_to(&self, new_right: f32) -> Rect;
    fn translate_top_to(&self, new_top: f32) -> Rect;
    fn translate_bottom_to(&self, new_bottom: f32) -> Rect;
    fn corners(&self) -> [Pos2; 4];
    fn rotated_corners(&self, angle: f32) -> [Pos2; 4];
    fn center_within(&self, rect: Rect) -> Rect;
    fn fit_and_center_within(&self, rect: Rect) -> Rect;
    fn with_aspect_ratio(&self, aspect_ratio: f32) -> Rect;
    fn _default_uv() -> Rect;
    fn _intersects(&self, other: Rect) -> bool;
}

impl RectExt for Rect {
    fn _constrain_to(&self, rect: Rect) -> Rect {
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

    fn rotate_bb_around_point(&self, angle: f32, point: Pos2) -> Rect {
        let origin = point;
        let top_left = self.min.to_vec2();
        let top_right = Pos2::new(self.max.x, self.min.y).to_vec2();
        let bottom_left = Pos2::new(self.min.x, self.max.y).to_vec2();
        let bottom_right = self.max.to_vec2();

        let rotated_top_left = Pos2::new(
            angle.cos() * (top_left.x - origin.x) - angle.sin() * (top_left.y - origin.y)
                + origin.x,
            angle.sin() * (top_left.x - origin.x)
                + angle.cos() * (top_left.y - origin.y)
                + origin.y,
        );

        let rotated_top_right = Pos2::new(
            angle.cos() * (top_right.x - origin.x) - angle.sin() * (top_right.y - origin.y)
                + origin.x,
            angle.sin() * (top_right.x - origin.x)
                + angle.cos() * (top_right.y - origin.y)
                + origin.y,
        );

        let rotated_bottom_left = Pos2::new(
            angle.cos() * (bottom_left.x - origin.x) - angle.sin() * (bottom_left.y - origin.y)
                + origin.x,
            angle.sin() * (bottom_left.x - origin.x)
                + angle.cos() * (bottom_left.y - origin.y)
                + origin.y,
        );

        let rotated_bottom_right = Pos2::new(
            angle.cos() * (bottom_right.x - origin.x) - angle.sin() * (bottom_right.y - origin.y)
                + origin.x,
            angle.sin() * (bottom_right.x - origin.x)
                + angle.cos() * (bottom_right.y - origin.y)
                + origin.y,
        );

        let rotated_corners = [
            rotated_top_left,
            rotated_top_right,
            rotated_bottom_left,
            rotated_bottom_right,
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

    fn _scale(&self, scale: f32) -> Rect {
        let center = self.center();
        let half_size = self.size() / 2.0;
        let new_half_size = half_size * scale;
        Rect::from_min_max(center - new_half_size, center + new_half_size)
    }

    fn translate_left_to(&self, new_left: f32) -> Rect {
        let mut translated = *self;
        let diff = new_left - translated.left();
        translated = translated.translate(Vec2::new(diff, 0.0));
        translated
    }

    fn translate_right_to(&self, new_right: f32) -> Rect {
        let mut translated = *self;
        let diff = new_right - translated.right();
        translated = translated.translate(Vec2::new(diff, 0.0));
        translated
    }

    fn translate_top_to(&self, new_top: f32) -> Rect {
        let mut translated = *self;
        let diff = new_top - translated.top();
        translated = translated.translate(Vec2::new(0.0, diff));
        translated
    }

    fn translate_bottom_to(&self, new_bottom: f32) -> Rect {
        let mut translated = *self;
        let diff = new_bottom - translated.bottom();
        translated = translated.translate(Vec2::new(0.0, diff));
        translated
    }

    fn corners(&self) -> [Pos2; 4] {
        [
            Pos2::new(self.left(), self.top()),
            Pos2::new(self.right(), self.top()),
            Pos2::new(self.left(), self.bottom()),
            Pos2::new(self.right(), self.bottom()),
        ]
    }

    fn rotated_corners(&self, angle: f32) -> [Pos2; 4] {
        let center = self.center();
        let corners = self.corners();
        let rot = Rot2::from_angle(angle);

        corners.map(|corner| {
            let offset = corner - center;
            let rotated_offset = rot * offset;
            center + rotated_offset
        })
    }

    fn center_within(&self, rect: Rect) -> Rect {
        let center = rect.center();
        let half_size = self.size() / 2.0;
        Rect::from_min_max(center - half_size, center + half_size)
    }

    fn fit_and_center_within(&self, rect: Rect) -> Rect {
        let aspect_ratio = self.width() / self.height();
        let rect_aspect_ratio = rect.width() / rect.height();
        if aspect_ratio > rect_aspect_ratio {
            // Scale to fit the width
            let new_width = rect.width();
            let new_height = new_width / aspect_ratio;
            let new_size = Vec2::new(new_width, new_height);
            let new_min = rect.center() - new_size / 2.0;
            Rect::from_min_size(new_min, new_size)
        } else {
            // Scale to fit the height
            let new_height = rect.height();
            let new_width = new_height * aspect_ratio;
            let new_size = Vec2::new(new_width, new_height);
            let new_min = rect.center() - new_size / 2.0;
            Rect::from_min_size(new_min, new_size)
        }
    }

    fn with_aspect_ratio(&self, aspect_ratio: f32) -> Rect {
        // Scale down to fit the aspect ratio
        let current_aspect_ratio = self.width() / self.height();
        if current_aspect_ratio > aspect_ratio {
            // Scale to fit the width
            let new_width = self.width();
            let new_height = new_width / aspect_ratio;
            let new_size = Vec2::new(new_width, new_height);
            let new_min = self.center() - new_size / 2.0;
            Rect::from_min_size(new_min, new_size)
        } else {
            // Scale to fit the height
            let new_height = self.height();
            let new_width = new_height * aspect_ratio;
            let new_size = Vec2::new(new_width, new_height);
            let new_min = self.center() - new_size / 2.0;
            Rect::from_min_size(new_min, new_size)
        }
    }

    fn _default_uv() -> Rect {
        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0))
    }

    fn _intersects(&self, other: Rect) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
    }
}

pub trait Vec2Ext {
    fn rotate_around(&self, center: Vec2, angle: f32) -> Vec2;
}

impl Vec2Ext for Vec2 {
    fn rotate_around(&self, center: Vec2, angle: f32) -> Vec2 {
        let x = self.x - center.x;
        let y = self.y - center.y;
        let cos = angle.cos();
        let sin = angle.sin();
        let new_x = x * cos - y * sin;
        let new_y = x * sin + y * cos;
        Vec2::new(new_x + center.x, new_y + center.y)
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

pub trait EditableValueTextEdit {
    fn text_edit_editable_value_singleline<T>(&mut self, value: &mut EditableValue<T>) -> T
    where
        T: Display,
        T: FromStr,
        T: Clone;

    /// Shows an editable value and reports valid live changes together with
    /// the underlying widget response.
    fn text_edit_editable_value_singleline_live<T>(
        &mut self,
        value: &mut EditableValue<T>,
    ) -> EditableValueTextResponse<T>
    where
        T: Display,
        T: FromStr,
        T: Clone;
}

pub struct EditableValueTextResponse<T> {
    pub value: Option<T>,
    pub response: Response,
}

impl EditableValueTextEdit for Ui {
    fn text_edit_editable_value_singleline<T>(&mut self, value: &mut EditableValue<T>) -> T
    where
        T: Display,
        T: FromStr,
        T: Clone,
    {
        let response = self.styled_text_edit_singleline(value.editable_value());
        if response.gained_focus() {
            value.begin_editing();
        } else if value.is_editing() && !response.has_focus() {
            value.end_editing();
        }
        value.value()
    }

    fn text_edit_editable_value_singleline_live<T>(
        &mut self,
        value: &mut EditableValue<T>,
    ) -> EditableValueTextResponse<T>
    where
        T: Display,
        T: FromStr,
        T: Clone,
    {
        let text_edit_response = self.styled_text_edit_singleline(value.editable_value());

        if text_edit_response.gained_focus() {
            value.begin_editing();
        }

        let parsed_value = text_edit_response
            .changed()
            .then(|| value.update_from_editable())
            .flatten();

        let committed = value.is_editing() && !text_edit_response.has_focus();
        if committed {
            value.end_editing();
        }

        EditableValueTextResponse {
            value: parsed_value,
            response: text_edit_response,
        }
    }
}

pub trait StyledTextEdit {
    fn styled_text_edit_singleline<S: TextBuffer>(&mut self, text: &mut S) -> Response;

    fn styled_text_edit_singleline_with<'text, S: TextBuffer>(
        &mut self,
        text: &'text mut S,
        configure: impl FnOnce(TextEdit<'text>) -> TextEdit<'text>,
    ) -> Response;
}

impl StyledTextEdit for Ui {
    fn styled_text_edit_singleline<S: TextBuffer>(&mut self, text: &mut S) -> Response {
        self.styled_text_edit_singleline_with(text, |text_edit| text_edit)
    }

    fn styled_text_edit_singleline_with<'text, S: TextBuffer>(
        &mut self,
        text: &'text mut S,
        configure: impl FnOnce(TextEdit<'text>) -> TextEdit<'text>,
    ) -> Response {
        self.add(configure(
            TextEdit::singleline(text).margin(style::TEXT_EDIT_MARGIN),
        ))
    }
}

pub trait IdExt {
    fn random() -> Id;
}

impl IdExt for Id {
    fn random() -> Id {
        Id::new(rand::random::<u64>())
    }
}

pub trait EguiUiExt {
    fn clickable<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>;
    fn both_centered<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>;
    fn sized<'a>(
        &mut self,
        id: Id,
        add_contents: impl FnMut(&mut Ui) + 'a,
        layout: impl FnOnce(&mut Ui, Vec2, Box<dyn FnOnce(&mut Ui) + 'a>),
    );
    fn child<R>(
        &mut self,
        max_rect: Rect,
        layout: Layout,
        sense: Sense,
        add_content: impl FnOnce(&mut Ui) -> R,
    ) -> R;
}

impl EguiUiExt for Ui {
    fn clickable<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        let response = self.allocate_ui(self.max_rect().size(), add_contents);

        if response.response.contains_pointer() {
            dep_mut!(CursorManager, |cursor_manager| {
                cursor_manager.set_cursor(egui::CursorIcon::PointingHand);
            });
        }

        InnerResponse::new(
            response.inner,
            self.interact(response.response.rect, self.next_auto_id(), Sense::click()),
        )
    }

    fn both_centered<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        let centered_layout = Layout {
            main_dir: egui::Direction::TopDown,
            main_wrap: true,
            main_align: Align::Center,
            main_justify: true,
            cross_align: Align::Center,
            cross_justify: false,
        };

        self.with_layout(centered_layout, add_contents)
    }

    fn sized<'a>(
        &mut self,
        id: Id,
        add_contents: impl FnMut(&mut Ui) + 'a,
        layout: impl FnOnce(&mut Ui, Vec2, Box<dyn FnOnce(&mut Ui) + 'a>),
    ) {
        dep_mut!(SizingManager, |sizing_manager| {
            sizing_manager.sized(self, id, add_contents, layout);
        });
    }

    fn child<R>(
        &mut self,
        max_rect: Rect,
        layout: Layout,
        sense: Sense,
        add_content: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        let mut child_ui = self.new_child(egui::UiBuilder::new().max_rect(max_rect).layout(layout));
        self.interact(max_rect, child_ui.id(), sense);
        add_content(&mut child_ui)
    }
}

pub trait ExifDateTimeExt {
    fn to_chrono_date_time(&self) -> Result<chrono::DateTime<Utc>, ParseError>;
}

impl ExifDateTimeExt for exif::DateTime {
    fn to_chrono_date_time(&self) -> Result<chrono::DateTime<Utc>, ParseError> {
        let naive_datetime =
            chrono::NaiveDateTime::parse_from_str(&self.to_string(), "%Y-%m-%d %H:%M:%S")?;
        let datetime = Utc.from_utc_datetime(&naive_datetime);
        Ok(datetime)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Color32, ComboBox};
    use egui_kittest::Harness;

    #[test]
    fn styled_text_inputs_match_dropdown_height() {
        let mut value = String::new();
        let mut harness = Harness::new_ui(|ui| {
            ui.spacing_mut().button_padding = egui::vec2(12.0, 6.0);
            ui.spacing_mut().interact_size.y = style::CONTROL_HEIGHT;

            let text_edit = ui.styled_text_edit_singleline(&mut value);
            let combo_box = ComboBox::from_id_salt("height_test")
                .selected_text("Value")
                .show_ui(ui, |_| {});

            assert_eq!(text_edit.rect.height(), combo_box.response.rect.height());
        });

        harness.run();
    }

    #[test]
    fn clip_mesh_constrains_vertices_to_rect() {
        let mut mesh = Mesh::default();
        mesh.add_colored_rect(
            Rect::from_min_max(Pos2::new(-10.0, -10.0), Pos2::new(20.0, 20.0)),
            Color32::WHITE,
        );
        let clip_rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(10.0));

        let clipped = mesh.clip_mesh(clip_rect);

        assert!(clipped.is_valid());
        assert!(!clipped.is_empty());
        assert_eq!(clipped.texture_id, mesh.texture_id);
        assert!(
            clipped
                .vertices
                .iter()
                .all(|vertex| clip_rect.expand(0.001).contains(vertex.pos))
        );
        assert_eq!(clipped.calc_bounds(), clip_rect);
    }

    #[test]
    fn clip_mesh_returns_original_mesh_when_fully_inside() {
        let mut mesh = Mesh::default();
        mesh.add_colored_rect(
            Rect::from_min_size(Pos2::new(2.0, 2.0), Vec2::splat(4.0)),
            Color32::WHITE,
        );

        assert_eq!(
            mesh.clip_mesh(Rect::from_min_size(Pos2::ZERO, Vec2::splat(10.0))),
            mesh
        );
    }

    #[test]
    fn clip_mesh_rejects_non_positive_rect() {
        let mut mesh = Mesh::default();
        mesh.add_colored_rect(
            Rect::from_min_size(Pos2::ZERO, Vec2::splat(10.0)),
            Color32::WHITE,
        );

        assert!(mesh.clip_mesh(Rect::ZERO).is_empty());
    }

    #[test]
    fn clipped_vertex_color_uses_gamma_space_interpolation() {
        let start_color = Color32::from_rgba_premultiplied(20, 40, 60, 80);
        let end_color = Color32::from_rgba_premultiplied(100, 120, 140, 160);
        let start = Vertex::untextured(Pos2::ZERO, start_color);
        let end = Vertex::untextured(Pos2::new(10.0, 0.0), end_color);

        let midpoint = vertex_at_x(start, end, 5.0);

        assert_eq!(
            midpoint.color,
            Color32::from_rgba_premultiplied(60, 80, 100, 120)
        );
    }
}
