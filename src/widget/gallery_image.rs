use eframe::{
    egui::{load::SizedTexture, Image, Response, Sense, Ui, Widget},
    epaint::{Color32, Vec2},
};
use log::error;

use crate::{
    photo::Photo, utils::Truncate,
    widget::placeholder::RectPlaceholder,
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
                //ui.spacing_mut().item_spacing = Vec2 { x: 10.0, y: 10.0 };

                let size = ui.available_size();

                let image_size = match self.photo.max_dimension() {
                    crate::photo::MaxPhotoDimension::Width => Vec2::new(
                        size.x,
                        self.photo.metadata.height() as f32
                            / self.photo.metadata.width() as f32
                            * size.x,
                    ),
                    crate::photo::MaxPhotoDimension::Height => Vec2::new(
                        self.photo.metadata.width() as f32
                            / self.photo.metadata.height() as f32
                            * size.y,
                        size.y,
                    ),
                };

                let (rect, response) = ui.allocate_exact_size(size, Sense::click());

                ui.allocate_ui_at_rect(rect, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(0.0);

                    if self.selected {
                        ui.painter().rect_filled(
                            ui.max_rect(),
                            6.0,
                            Color32::from_rgb(15, 15, 180),
                        );
                    } else {
                        ui.painter()
                            .rect_filled(ui.max_rect(), 6.0, Color32::from_rgb(15, 15, 15));
                    }

                    ui.vertical_centered(|ui| {
                        ui.add_space(10.0);

                        ui.horizontal(|ui| {
                            ui.add_space(10.0);

                            ui.label(
                                self.photo
                                    .path
                                    .file_name()
                                    .map(|file_name| file_name.to_string_lossy().truncate(30))
                                    .unwrap_or_else(|| {
                                        self.photo.path.to_string_lossy().truncate(30)
                                    }),
                            );
                        });

                        ui.vertical_centered(|ui| {
                            let available_size = ui.available_size();
                            let width_scale = available_size.x / image_size.x;
                            let height_scale = available_size.y / image_size.y;
                            let scale: f32 = width_scale.min(height_scale);
                            let scaled_image_size = image_size * scale * if self.photo.metadata.rotation().is_horizontal() { 0.9 } else { 0.75 };

                            let verical_spacing = (available_size.y - scaled_image_size.y) / 2.0;

                            ui.add_space(verical_spacing);
                            
                            match self.texture {
                                Ok(Some(texture)) => {
                                    let rotation = self.photo.metadata.rotation();
                                    ui.add(
                                        Image::from_texture(texture)
                                            .rotate(rotation.radians(), Vec2::splat(0.5))
                                            .fit_to_exact_size(scaled_image_size),
                                    );
                                }
                                Ok(None) => {
                                    RectPlaceholder::new(
                                        scaled_image_size,
                                        Color32::from_rgb(50, 50, 50),
                                    )
                                    .ui(ui);
                                }
                                Err(err) => {
                                    // Show red square for error for now
                                    // TODO: Show error message or something
                                    RectPlaceholder::new(
                                        scaled_image_size,
                                        Color32::from_rgb(255, 0, 0),
                                    )
                                    .ui(ui);
                                    error!(
                                        "Failed to load image: {:?}. {:?}",
                                        self.photo.path, err
                                    );
                                }
                            }
                        });
                    });
                });

                response
            },
        );

        response.inner
    }
}
