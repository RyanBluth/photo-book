use eframe::{
    egui::{Image, Response, Sense, Ui, Widget, load::SizedTexture},
    epaint::Vec2,
};
use egui::{Spinner, Stroke, StrokeKind, UiBuilder};
use log::error;

use crate::{
    dep, dep_mut, gpu_photo_adjustment::GpuPhotoAdjustmentRenderer, photo::Photo,
    photo_manager::PhotoManager, theme::color, widget::placeholder::RectPlaceholder,
};

pub struct GalleryImage {
    photo: Photo,
    texture: anyhow::Result<Option<SizedTexture>>,
    selected: bool,
}

impl GalleryImage {
    pub fn new(
        photo: Photo,
        texture: anyhow::Result<Option<SizedTexture>>,
        selected: bool,
    ) -> Self {
        Self {
            photo,
            texture,
            selected,
        }
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

                        let image_rect = match self.texture {
                            Ok(Some(texture)) => {
                                let adjustments = self.photo.adjustments();
                                let (image_rect, _) =
                                    ui.allocate_exact_size(scaled_image_size, Sense::hover());
                                let gpu_painted = !adjustments.is_identity()
                                    && dep!(GpuPhotoAdjustmentRenderer, |renderer| renderer
                                        .try_paint_thumbnail(
                                            ui,
                                            &self.photo,
                                            rect,
                                            image_rect,
                                            &adjustments,
                                        ));

                                if !gpu_painted {
                                    let fallback_texture = if adjustments.is_identity() {
                                        Some(texture)
                                    } else {
                                        dep_mut!(PhotoManager, |photo_manager| {
                                            photo_manager
                                                .thumbnail_texture_for(&self.photo, ui.ctx())
                                        })
                                        .ok()
                                        .flatten()
                                        .or(Some(texture))
                                    };

                                    if let Some(texture) = fallback_texture {
                                        Image::from_texture(texture).paint_at(ui, image_rect);
                                    } else {
                                        ui.painter().rect_filled(
                                            image_rect,
                                            4.0,
                                            color::SURFACE_MUTED,
                                        );
                                    }
                                };

                                ui.painter().rect_stroke(
                                    image_rect,
                                    5.0,
                                    Stroke::new(5.0, color::SURFACE_X_DARK),
                                    StrokeKind::Outside,
                                );
                                image_rect
                            }
                            Ok(None) => {
                                let response = RectPlaceholder::new(
                                    scaled_image_size,
                                    color::SURFACE_MUTED,
                                    4.0,
                                )
                                .ui(ui);

                                ui.put(response.rect, Spinner::new());
                                response.rect
                            }
                            Err(err) => {
                                // Show themed error placeholder for now.
                                // TODO: Show error message or something
                                let rect =
                                    RectPlaceholder::new(scaled_image_size, color::ERROR, 4.0)
                                        .ui(ui)
                                        .rect;
                                error!("Failed to load image: {:?}. {:?}", self.photo.path, err);
                                rect
                            }
                        };

                        if self.selected {
                            ui.painter().rect_stroke(
                                image_rect.expand(3.0),
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
