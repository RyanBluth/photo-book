use eframe::{
    egui::{Image, Response, Sense, Ui, Widget, load::SizedTexture},
    epaint::{Color32, Rect, Vec2},
};
use log::info;

use crate::{photo::Photo, widget::placeholder::RectPlaceholder, dependencies::{Dependency, SingletonFor}, event_bus::{EventBus, GalleryImageEvent}};

pub struct GalleryImage {
    photo: Photo,
    texture: anyhow::Result<Option<SizedTexture>>,
}

impl GalleryImage {
    pub const SIZE: Vec2 = Vec2 { x: 256.0, y: 256.0 };
    pub const IMAGE_SIZE: Vec2 = Vec2 {
        x: Self::SIZE.x,
        y: Self::SIZE.y * 0.75,
    };

    pub fn new(photo: Photo, texture: anyhow::Result<Option<SizedTexture>>) -> Self {
        Self { photo, texture }
    }
}

impl Widget for GalleryImage {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.spacing_mut().item_spacing = Vec2 { x: 10.0, y: 10.0 };

        let mut response = ui
            .allocate_ui(Self::SIZE, |ui| {

                ui.vertical(|ui| {
                    ui.set_min_size(Self::SIZE);

                    match self.texture {
                        Ok(Some(texture)) => {
                            ui.add(
                                Image::from_texture(texture)
                                    .fit_to_exact_size(Self::IMAGE_SIZE)
                            );
                        }
                        Ok(None) => {
                            RectPlaceholder::new(Self::IMAGE_SIZE, Color32::from_rgb(50, 50, 50))
                                .ui(ui);
                        }
                        Err(_) => {
                            // Show red square for error for now
                            // TODO: Show error message or something
                            RectPlaceholder::new(Self::IMAGE_SIZE, Color32::from_rgb(255, 0, 0))
                                .ui(ui);
                        }
                    }

                    ui.label(self.photo.file_name());
                });
            })
            .response;

        response = response.interact(Sense::click());

        if response.clicked() {
            info!("Clicked on image: {:?}", self.photo);
            Dependency::<EventBus<GalleryImageEvent>>::get().emit(GalleryImageEvent::Selected(self.photo.clone()));
        }

        response
    }
}
