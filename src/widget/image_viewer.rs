use eframe::{
    egui::{self, Image, Key, Response, Sense, Widget},
    emath::Rot2,
    epaint::{Color32, Mesh, Pos2, Rect, Shape, Vec2},
};

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    photo::{
        MaxPhotoDimension::{Height, Width},
        Photo,
    },
    photo_manager::PhotoManager,
    theme::color,
    utils::RectExt,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ImageViewerState {
    pub scale: f32,
    pub offset: Vec2,
}

impl Default for ImageViewerState {
    fn default() -> Self {
        Self {
            scale: 1.0,
            offset: Vec2::ZERO,
        }
    }
}

pub enum Request {
    Exit,
    Previous,
    Next,
}

pub struct ImageViewerResponse {
    pub request: Option<Request>,
    pub response: Response,
}

pub struct ImageViewer<'a> {
    photo: &'a Photo,
    state: &'a mut ImageViewerState,
    photo_manager: Singleton<PhotoManager>,
}

impl<'a> ImageViewer<'a> {
    pub fn new(photo: &'a Photo, state: &'a mut ImageViewerState) -> Self {
        Self {
            photo,
            state,
            photo_manager: Dependency::<PhotoManager>::get(),
        }
    }

    pub fn show(self, ui: &mut eframe::egui::Ui) -> ImageViewerResponse {
        let response = self.ui(ui);

        let mut viewer_response = ImageViewerResponse {
            request: None,
            response,
        };

        if ui.input(|input| input.key_pressed(Key::Escape)) {
            viewer_response.request = Some(Request::Exit);
        } else if ui.input(|input| input.key_pressed(Key::ArrowLeft)) {
            viewer_response.request = Some(Request::Previous);
        } else if ui.input(|input| input.key_pressed(Key::ArrowRight)) {
            viewer_response.request = Some(Request::Next);
        }

        viewer_response
    }

    fn translate_from_center(offset: Vec2, rect: Rect, relative_to: Rect) -> Rect {
        let mut new_rect = rect;

        new_rect.set_center(Pos2::new(
            relative_to.center().x + offset.x,
            relative_to.center().y + offset.y,
        ));

        new_rect
    }
}

impl<'a> Widget for ImageViewer<'a> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> Response {
        let available_size = ui.available_size();

        let (rect, response) = ui.allocate_exact_size(available_size, Sense::click_and_drag());

        ui.painter().rect_filled(rect, 0.0, Color32::BLACK);

        response.request_focus();

        let mut image_rect = rect;

        // Adjust the rect aspect ratio
        match self.photo.max_dimension() {
            Width => {
                let aspect_ratio = self.photo.metadata.rotated_height() as f32
                    / self.photo.metadata.rotated_width() as f32;
                if image_rect.width() > image_rect.height() {
                    let desired_height = (image_rect.width() * aspect_ratio).min(available_size.y);
                    let adjusted_width = desired_height
                        * (self.photo.metadata.rotated_width() as f32
                            / self.photo.metadata.rotated_height() as f32);

                    image_rect = Rect::from_center_size(
                        rect.center(),
                        Vec2::new(adjusted_width, desired_height),
                    );
                } else {
                    let desired_width = (image_rect.height() / aspect_ratio).min(available_size.x);
                    let adjusted_height = desired_width
                        * (self.photo.metadata.rotated_height() as f32
                            / self.photo.metadata.rotated_width() as f32);

                    image_rect = Rect::from_center_size(
                        rect.center(),
                        Vec2::new(desired_width, adjusted_height),
                    );
                }
            }
            Height => {
                let aspect_ratio = self.photo.metadata.rotated_width() as f32
                    / self.photo.metadata.rotated_height() as f32;
                if image_rect.width() > image_rect.height() {
                    let desired_height = (image_rect.width() * aspect_ratio).min(available_size.y);
                    let adjusted_width = desired_height
                        * (self.photo.metadata.rotated_width() as f32
                            / self.photo.metadata.rotated_height() as f32);

                    image_rect = Rect::from_center_size(
                        rect.center(),
                        Vec2::new(adjusted_width, desired_height),
                    );
                } else {
                    let desired_width = (image_rect.height() / aspect_ratio).min(available_size.x);
                    let adjusted_height = desired_width
                        * (self.photo.metadata.rotated_height() as f32
                            / self.photo.metadata.rotated_width() as f32);

                    image_rect = Rect::from_center_size(
                        rect.center(),
                        Vec2::new(desired_width, adjusted_height),
                    );
                }
            }
        };

        image_rect = Self::translate_from_center(self.state.offset, image_rect, rect);

        ui.input(|i| {
            if i.pointer.hover_pos().is_some() && i.raw_scroll_delta.y != 0.0 {
                let mouse_pos = i.pointer.hover_pos().unwrap();

                let rel_mouse_pos_before = image_rect.center() - mouse_pos;

                let mut scale_delta = 1.0;

                if i.raw_scroll_delta.y > 0.0 {
                    scale_delta = 1.1;
                } else if i.raw_scroll_delta.y < 0.0 {
                    scale_delta = 0.9;
                }

                self.state.scale *= scale_delta;

                let scaled_width_diff = image_rect.width() * self.state.scale - image_rect.width();
                let scaled_height_diff =
                    image_rect.height() * self.state.scale - image_rect.height();

                image_rect = image_rect
                    .expand2(Vec2::new(scaled_width_diff * 0.5, scaled_height_diff * 0.5));

                let rel_mouse_pos_after = rel_mouse_pos_before * scale_delta;

                self.state.offset += rel_mouse_pos_after - rel_mouse_pos_before;
            } else {
                let scaled_width_diff = image_rect.width() * self.state.scale - image_rect.width();
                let scaled_height_diff =
                    image_rect.height() * self.state.scale - image_rect.height();

                image_rect = image_rect
                    .expand2(Vec2::new(scaled_width_diff * 0.5, scaled_height_diff * 0.5));
            }
        });

        image_rect = Self::translate_from_center(self.state.offset, image_rect, rect);

        let image_rect_size = image_rect.size();

        // Adjust image_rect so it always fills rect, or is centered in rect
        if image_rect.width() >= rect.width() {
            self.state.offset.x += response.drag_delta().x;
            image_rect = Self::translate_from_center(self.state.offset, image_rect, rect);

            if image_rect.right() < rect.right() {
                image_rect.set_right(rect.right());
                image_rect.set_left(rect.right() - image_rect_size.x);

                self.state.offset.x = image_rect.center().x - rect.center().x;
            } else if image_rect.left() > rect.left() {
                image_rect.set_left(rect.left());
                image_rect.set_right(rect.left() + image_rect_size.x);

                self.state.offset.x = image_rect.center().x - rect.center().x;
            }
        }

        if image_rect.height() >= rect.height() {
            self.state.offset.y += response.drag_delta().y;
            image_rect = Self::translate_from_center(self.state.offset, image_rect, rect);

            if image_rect.bottom() < rect.bottom() {
                image_rect.set_bottom(rect.bottom());
                image_rect.set_top(rect.bottom() - image_rect_size.y);

                self.state.offset.y = image_rect.center().y - rect.center().y;
            } else if image_rect.top() > rect.top() {
                image_rect.set_top(rect.top());
                image_rect.set_bottom(rect.top() + image_rect_size.y);

                self.state.offset.y = image_rect.center().y - rect.center().y;
            }
        }

        if image_rect.width() <= rect.width() {
            image_rect.set_center(Pos2::new(rect.center().x, image_rect.center().y));
            self.state.offset.x = 0.0;
        }

        if image_rect.height() <= rect.height() {
            image_rect.set_center(Pos2::new(image_rect.center().x, rect.center().y));
            self.state.offset.y = 0.0;
        }

        image_rect = Self::translate_from_center(self.state.offset, image_rect, rect);

        // If the image is rotated then swap the dimensions because we want egui to rotate the image when drawing
        if self.photo.metadata.width() != self.photo.metadata.rotated_width() {
            image_rect = Rect::from_center_size(
                image_rect.center(),
                Vec2::new(image_rect.height(), image_rect.width()),
            );
        }

        match self
            .photo_manager
            .with_lock_mut(|photo_manager| photo_manager.texture_for(self.photo, ui.ctx()))
        {
            Ok(Some(texture)) => {
                let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2 { x: 1.0, y: 1.0 });

                let painter = ui.painter().with_clip_rect(rect);
                let mut mesh = Mesh::with_texture(texture.id);
                mesh.add_rect_with_uv(image_rect, uv, Color32::WHITE);
                mesh.rotate(
                    Rot2::from_angle(self.photo.metadata.rotation().radians()),
                    image_rect.min + Vec2::splat(0.5) * image_rect.size(),
                );
                painter.add(Shape::mesh(mesh));
            }
            Ok(None) => match self.photo_manager.with_lock_mut(|photo_manager| {
                photo_manager.thumbnail_texture_for(self.photo, ui.ctx())
            }) {
                Ok(Some(texture)) => {
                    Image::from_texture(texture)
                        .rotate(self.photo.metadata.rotation().radians(), Vec2::splat(0.5))
                        .paint_at(ui, image_rect);
                }
                Ok(None) | Err(_) => {
                    ui.painter().rect_filled(
                        image_rect
                            .rotate_bb_around_center(self.photo.metadata.rotation().radians()),
                        0.0,
                        color::PLACEHOLDER,
                    );
                }
            },
            Err(error) => {
                ui.painter().text(
                    image_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("Error: {}", error),
                    egui::FontId::default(),
                    Color32::from_rgb(255, 0, 0),
                );
            }
        }

        response
    }
}
