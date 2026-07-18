use eframe::{
    egui::{Response, Sense, Ui, Widget},
    epaint::Vec2,
};
use egui::{Spinner, Stroke, StrokeKind, UiBuilder};
use log::error;

use crate::{
    photo::Photo,
    photo_renderer::{PhotoRenderOptions, PhotoRenderStatus, PhotoRenderer},
    theme::color,
};

pub struct GalleryImage {
    photo: Photo,
    selected: bool,
}

impl GalleryImage {
    pub fn new(photo: Photo, selected: bool) -> Self {
        Self { photo, selected }
    }
}

impl Widget for GalleryImage {
    fn ui(self, ui: &mut Ui) -> Response {
        let response = ui.push_id(
            format!("GalleryImage_{}", self.photo.path.to_string_lossy()),
            |ui| {
                let size = ui.available_size();
                let image_size = fitted_image_size(&self.photo, size);

                let (rect, response) = ui.allocate_exact_size(size, Sense::click());

                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(0.0);

                    ui.vertical_centered(|ui| {
                        let available_size = ui.available_size();
                        let width_scale = available_size.x / image_size.x;
                        let height_scale = available_size.y / image_size.y;
                        let scale: f32 = width_scale.min(height_scale);
                        let scaled_image_size: Vec2 = image_size * scale;

                        let verical_spacing =
                            (0.0 as f32).max((available_size.y - scaled_image_size.y) / 2.0);

                        ui.add_space(verical_spacing);

                        let (image_rect, _) =
                            ui.allocate_exact_size(scaled_image_size, Sense::hover());
                        let adjustments = self.photo.adjustments();
                        match PhotoRenderer::paint(
                            ui,
                            &self.photo,
                            &adjustments,
                            image_rect,
                            PhotoRenderOptions::default()
                                .thumbnail()
                                .with_clip_rect(rect)
                                .with_render_key("image-gallery"),
                        ) {
                            Ok(PhotoRenderStatus::Pending) => {
                                ui.painter()
                                    .rect_filled(image_rect, 4.0, color::SURFACE_MUTED);
                                ui.put(image_rect, Spinner::new());
                            }
                            Err(err) => {
                                ui.painter().rect_filled(image_rect, 4.0, color::ERROR);
                                error!("Failed to load image: {:?}. {:?}", self.photo.path, err);
                            }
                            Ok(_) => {}
                        }

                        ui.painter().rect_stroke(
                            image_rect,
                            5.0,
                            Stroke::new(5.0, color::SURFACE_X_DARK),
                            StrokeKind::Outside,
                        );

                        if self.selected {
                            ui.painter().rect_stroke(
                                image_rect,
                                6.0,
                                Stroke::new(3.0, color::ACCENT),
                                StrokeKind::Inside,
                            );
                        }
                    });
                });

                response
            },
        );

        response.inner
    }
}

fn fitted_image_size(photo: &Photo, available_size: Vec2) -> Vec2 {
    let image_size = Vec2::new(
        photo.metadata.rotated_width() as f32,
        photo.metadata.rotated_height() as f32,
    );
    if image_size.x <= 0.0 || image_size.y <= 0.0 || image_size.x.is_nan() || image_size.y.is_nan()
    {
        return available_size - Vec2::splat(20.0);
    }

    let scale = (available_size.x / image_size.x).min(available_size.y / image_size.y);
    image_size * scale
}
