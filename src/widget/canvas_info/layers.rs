use eframe::{
    egui::{text, Grid, Image, Widget},
    epaint::Vec2,
    epaint::{Color32, Rect},
};

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    photo::{self, Photo},
    photo_manager::PhotoManager,
    widget::{page_canvas::CanvasPhoto, placeholder::RectPlaceholder, spacer::Spacer},
};

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub photo: CanvasPhoto,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub selected: bool,
}

impl Layer {
    pub fn with_photo(photo: Photo, id: usize) -> Self {
        let name = photo.file_name().to_string();
        Self {
            photo: CanvasPhoto::new(photo, id),
            name: name,
            visible: true,
            locked: false,
            selected: false,
        }
    }
}

#[derive(Debug)]
pub struct Layers<'a> {
    layers: &'a mut Vec<Layer>,
    photo_manager: Singleton<PhotoManager>,
}

impl<'a> Layers<'a> {
    pub fn new(layers: &'a mut Vec<Layer>) -> Self {
        Self {
            layers,
            photo_manager: Dependency::get(),
        }
    }
}

impl<'a> Widget for Layers<'a> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> eframe::egui::Response {
        let selected_index = self
            .layers
            .iter()
            .position(|layer| layer.selected)
            .unwrap_or(usize::MAX);
        ui.allocate_ui(ui.available_size(), |ui| {
            Grid::new("LayersGrid")
                .num_columns(3)
                .with_row_color(move |row, _| {
                    if row > 0 && row - 2 == selected_index {
                        return Some(Color32::from_rgb(0, 0, 255));
                    }
                    None
                })
                .spacing(Vec2::new(0.0, 3.0))
                .show(ui, |ui| {
                    
                    ui.end_row();

                    for layer in self.layers {

                        let texture_id = self.photo_manager.with_lock_mut(|photo_manager| {
                            photo_manager.thumbnail_texture_for(&layer.photo.photo, ui.ctx())
                        });

                        let image_size = Vec2::from(layer.photo.photo.size_with_max_size(50.0));

                        match texture_id {
                            Ok(Some(texture_id)) => {
                                let image = Image::from_texture(texture_id)
                                    .rotate(
                                        layer.photo.photo.metadata.rotation().radians(),
                                        Vec2::splat(0.5),
                                    )
                                    .fit_to_exact_size(image_size);
                                ui.add(image);
                            }
                            _ => {
                                RectPlaceholder::new(image_size, Color32::GRAY);
                            }
                        };

                        ui.add_space(10.0);

                        ui.label(&layer.name);

                        ui.end_row();

                        ui.separator();
                        ui.separator();
                        ui.separator();

                        ui.end_row();
                    }
                })
                .response
        })
        .response
    }
}
