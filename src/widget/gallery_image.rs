use eframe::{
    egui::{load::SizedTexture, Image, Response, Sense, Ui, Widget},
    epaint::{Color32, Vec2},
};
use log::error;

use crate::{
    photo::Photo, photo_manager::PhotoLoadResult, utils::Truncate,
    widget::placeholder::RectPlaceholder,
};

pub struct GalleryImage {
    photo: PhotoLoadResult,
    texture: anyhow::Result<Option<SizedTexture>>,
    selected: bool,
}

impl GalleryImage {
    pub const SIZE: Vec2 = Vec2 { x: 256.0, y: 256.0 };

    pub fn new(
        photo: PhotoLoadResult,
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
        ui.spacing_mut().item_spacing = Vec2 { x: 10.0, y: 10.0 };

        let image_size = match &self.photo {
            PhotoLoadResult::Pending(_) => Vec2::new(Self::SIZE.x * 0.9, Self::SIZE.y * 0.9),
            PhotoLoadResult::Ready(photo) => match photo.max_dimension() {
                crate::photo::MaxPhotoDimension::Width => Vec2::new(
                    Self::SIZE.x * 0.9,
                    photo.metadata.rotated_height() as f32 / photo.metadata.rotated_width() as f32
                        * Self::SIZE.x
                        * 0.9,
                ),
                crate::photo::MaxPhotoDimension::Height => Vec2::new(
                    photo.metadata.rotated_width() as f32 / photo.metadata.rotated_height() as f32
                        * Self::SIZE.y,
                    Self::SIZE.y,
                ),
            },
        };

        let mut response = ui
            .allocate_ui(Self::SIZE, |ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(10.0);

                if self.selected {
                    ui.painter()
                        .rect_filled(ui.max_rect(), 6.0, Color32::from_rgb(15, 15, 180));
                } else {
                    ui.painter()
                        .rect_filled(ui.max_rect(), 6.0, Color32::from_rgb(15, 15, 15));
                }

                ui.vertical(|ui| {
                    ui.set_max_size(Self::SIZE);

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        ui.add_space(10.0);

                        ui.label(self.photo.path().display().truncate(30));
                    });

                    ui.centered_and_justified(|ui| {
                        match self.texture {
                            Ok(Some(texture)) => {
                                let rotation = match self.photo {
                                    PhotoLoadResult::Pending(_) => {
                                        crate::photo::PhotoRotation::Normal
                                    }
                                    PhotoLoadResult::Ready(photo) => photo.metadata.rotation(),
                                };

                                ui.add(
                                    Image::from_texture(texture)
                                        .rotate(rotation.radians(), Vec2::splat(0.5))
                                        .fit_to_exact_size(image_size),
                                );
                            }
                            Ok(None) => {
                                RectPlaceholder::new(image_size, Color32::from_rgb(50, 50, 50))
                                    .ui(ui);
                            }
                            Err(err) => {
                                // Show red square for error for now
                                // TODO: Show error message or something
                                RectPlaceholder::new(image_size, Color32::from_rgb(255, 0, 0))
                                    .ui(ui);
                                error!("Failed to load image: {:?}. {:?}", self.photo.path(), err);
                            }
                        }
                    });
                });
            })
            .response;

        response = response.interact(Sense::click());

        response
    }
}
