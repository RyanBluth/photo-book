use eframe::{
    egui::{load::SizedTexture, Image, Response, Sense, Ui, Widget},
    epaint::{Color32, Rect, Vec2},
};
use log::{error, info};

use crate::{
    dependencies::{Dependency, SingletonFor},
    photo::Photo,
    widget::placeholder::RectPlaceholder, utils::Truncate,
};

pub struct GalleryImage {
    photo: Photo,
    texture: anyhow::Result<Option<SizedTexture>>,
}

impl GalleryImage {
    pub const SIZE: Vec2 = Vec2 { x: 256.0, y: 256.0 };

    pub fn new(photo: Photo, texture: anyhow::Result<Option<SizedTexture>>) -> Self {
        Self { photo, texture }
    }
}

impl Widget for GalleryImage {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.spacing_mut().item_spacing = Vec2 { x: 10.0, y: 10.0 };

        let image_size = match self.photo.max_dimension() {
            crate::photo::MaxPhotoDimension::Width(_) => Vec2::new(
                Self::SIZE.x * 0.8,
                self.photo.metadata.rotated_height / self.photo.metadata.rotated_width
                    * Self::SIZE.x
                    * 0.8,
            ),
            crate::photo::MaxPhotoDimension::Height(_) => Vec2::new(
                self.photo.metadata.rotated_width / self.photo.metadata.rotated_height
                    * Self::SIZE.y
                    * 0.8,
                Self::SIZE.y * 0.8,
            ),
        };

        let mut response = ui
            .allocate_ui(Self::SIZE, |ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(10.0);

                ui.painter()
                    .rect_filled(ui.max_rect(), 6.0, Color32::from_rgb(15, 15, 15));

                ui.vertical(|ui| {
                    ui.set_max_size(Self::SIZE);

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        ui.add_space(10.0);

                        ui.label(self.photo.file_name().truncate(30));
                    });

                    ui.centered_and_justified(|ui| {
                        match self.texture {
                            Ok(Some(texture)) => {
                                ui.add(
                                    Image::from_texture(texture)
                                        .rotate(
                                            self.photo.metadata.rotation.radians(),
                                            Vec2::splat(0.5),
                                        )
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
                                error!("Failed to load image: {:?}. {:?}", self.photo.path, err);
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
