

use eframe::{
    egui::{Image, Response, Ui, Widget},
    epaint::{Color32, Vec2},
};

use crate::{photo::Photo, widget::placeholder::RectPlaceholder};

pub struct GalleryImage {
    photo: Photo,
    thumbnail_bytes: Option<Vec<u8>>,
}

impl GalleryImage {
    pub const SIZE: Vec2 = Vec2 { x: 256.0, y: 256.0 };
    pub const IMAGE_SIZE: Vec2 = Vec2 {
        x: Self::SIZE.x,
        y: Self::SIZE.y * 0.75,
    };

    pub fn new(photo: Photo) -> Self {
        Self { photo, thumbnail_bytes: None }
    }
}

impl Widget for GalleryImage {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.spacing_mut().item_spacing = Vec2 { x: 10.0, y: 10.0 };
        ui.allocate_ui(Self::SIZE, |ui| {
            ui.vertical(|ui| {
                ui.set_min_size(Self::SIZE);

                match self.photo.thumbnail() {
                    Some(bytes) => {
                        ui.add(
                            Image::from_bytes(self.photo.string_path(), bytes)
                                .fit_to_exact_size(Self::IMAGE_SIZE),
                        );
                    }
                    None => {
                        RectPlaceholder::new(Self::IMAGE_SIZE, Color32::from_rgb(50, 50, 50))
                            .ui(ui);
                    }
                }

                ui.label(self.photo.file_name());
            });
        })
        .response
    }
}
