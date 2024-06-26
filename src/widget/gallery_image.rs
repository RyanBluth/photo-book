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
            format!("GalleryImage_{}", self.photo.path.display()),
            |ui| {
                // let other = ui.allocate_response(Self::SIZE, Sense::click());

                ui.spacing_mut().item_spacing = Vec2 { x: 10.0, y: 10.0 };

                let size = ui.available_size();

                let image_size = match self.photo.max_dimension() {
                    crate::photo::MaxPhotoDimension::Width => Vec2::new(
                        size.x * 0.9,
                        self.photo.metadata.rotated_height() as f32
                            / self.photo.metadata.rotated_width() as f32
                            * size.x
                            * 0.9,
                    ),
                    crate::photo::MaxPhotoDimension::Height => Vec2::new(
                        self.photo.metadata.rotated_width() as f32
                            / self.photo.metadata.rotated_height() as f32
                            * size.y,
                        size.y,
                    ),
                };

                let (rect, response) = ui.allocate_exact_size(size, Sense::click());

                ui.allocate_ui_at_rect(rect, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(10.0);

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

                    ui.vertical(|ui| {
                        ui.set_max_size(size);

                        ui.add_space(10.0);

                        ui.horizontal(|ui| {
                            ui.add_space(10.0);

                            ui.label(self.photo.path.display().truncate(30));
                        });

                        ui.centered_and_justified(|ui| {
                            match self.texture {
                                Ok(Some(texture)) => {
                                    let rotation = self.photo.metadata.rotation();

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
                                    error!(
                                        "Failed to load image: {:?}. {:?}",
                                        self.photo.path,
                                        err
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
