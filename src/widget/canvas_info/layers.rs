use std::borrow::{Borrow, BorrowMut};

use eframe::{
    egui::{text, CursorIcon, Grid, Image, Sense, Style, Visuals, Widget},
    egui_glow::painter,
    epaint::Vec2,
    epaint::{Color32, Rect},
};

use crate::{
    cursor_manager::CursorManager,
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

pub struct LayersResponse {
    pub selected_layer: Option<usize>,
}

impl<'a> Layers<'a> {
    pub fn new(layers: &'a mut Vec<Layer>) -> Self {
        Self {
            layers,
            photo_manager: Dependency::get(),
        }
    }

    pub fn show(&mut self, ui: &mut eframe::egui::Ui) -> LayersResponse {
        let mut selected_layer = None;

        ui.vertical(|ui| {
            for layer in self.layers.iter_mut() {
                let layer_response= ui.horizontal(|ui| {

                    ui.set_height(60.0);

                    if layer.selected {
                        let painter = ui.painter();
                        painter.rect_filled(ui.max_rect(), 0.0, Color32::from_rgb(0, 0, 255));
                    }

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
                            ui.add_sized(Vec2::new(70.0, 50.0), image);
                        }
                        _ => {
                            ui.add_sized(
                                Vec2::new(70.0, 50.0),
                                RectPlaceholder::new(image_size, Color32::GRAY),
                            );
                        }
                    };

                    ui.label(&layer.name);

                    ui.add_space(10.0);

                    if ui.rect_contains_pointer(ui.max_rect()) {
                        Dependency::<CursorManager>::get().with_lock_mut(|cursor_manager| {
                            cursor_manager.set_cursor(CursorIcon::PointingHand);
                        });
                    }

                    if ui.input(|i| i.pointer.primary_clicked())
                        && ui.rect_contains_pointer(ui.max_rect())
                    {
                        layer.selected = true;
                        selected_layer = Some(layer.photo.id);
                    }
                });

                ui.separator();
            }
        });

        LayersResponse { selected_layer }
    }
}
// }

// impl<'a> Widget for Layers<'a> {
//     fn ui(self, ui: &mut eframe::egui::Ui) -> eframe::egui::Response {
//         ui.vertical(|ui| {
//             for layer in self.layers {
//                 let layer_response = ui.horizontal(|ui| {

//                     ui.set_height(60.0);

//                     if layer.selected {
//                         let painter = ui.painter();
//                         painter.rect_filled(ui.max_rect(), 0.0, Color32::from_rgb(0, 0, 255));
//                     }

//                     let texture_id = self.photo_manager.with_lock_mut(|photo_manager| {
//                         photo_manager.thumbnail_texture_for(&layer.photo.photo, ui.ctx())
//                     });

//                     let image_size = Vec2::from(layer.photo.photo.size_with_max_size(50.0));

//                     match texture_id {
//                         Ok(Some(texture_id)) => {
//                             let image = Image::from_texture(texture_id)
//                                 .rotate(
//                                     layer.photo.photo.metadata.rotation().radians(),
//                                     Vec2::splat(0.5),
//                                 )
//                                 .fit_to_exact_size(image_size);
//                             ui.add_sized(Vec2::new(70.0, 50.0), image);
//                         }
//                         _ => {
//                             ui.add_sized(
//                                 Vec2::new(70.0, 50.0),
//                                 RectPlaceholder::new(image_size, Color32::GRAY),
//                             );
//                         }
//                     };

//                     ui.label(&layer.name);

//                     ui.add_space(10.0);

//                     if ui.rect_contains_pointer(ui.max_rect()) {
//                         Dependency::<CursorManager>::get().with_lock_mut(|cursor_manager| {
//                             cursor_manager.set_cursor(CursorIcon::PointingHand);
//                         });
//                     }

//                     if ui.input(|i| i.pointer.primary_clicked())
//                         && ui.rect_contains_pointer(ui.max_rect())
//                     {
//                         layer.selected = true;
//                     }
//                 });

//                 ui.separator();
//             }
//         })
//         .response
//     }
// }
