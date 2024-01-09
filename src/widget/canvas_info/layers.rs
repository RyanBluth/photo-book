use eframe::egui::{Grid, Widget};

use crate::{photo::Photo, widget::page_canvas::CanvasPhoto};

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub photo: CanvasPhoto,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
}

impl Layer {
    pub fn with_photo(photo: Photo, id: usize) -> Self {
        let name = photo.file_name().to_string();
        Self {
            photo: CanvasPhoto::new(photo, id),
            name: name,
            visible: true,
            locked: false,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Layers<'a> {
    layers: &'a mut Vec<Layer>,
}

impl<'a> Layers<'a> {
    pub fn new(layers: &'a mut Vec<Layer>) -> Self {
        Self { layers }
    }
}

impl<'a> Widget for Layers<'a> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> eframe::egui::Response {
        ui.allocate_ui(ui.available_size(), |ui| {
            Grid::new("layers_grid")
            .striped(false)
            .num_columns(1)
            .spacing([10.0, 10.0])
            .show(ui, |ui| {
                for layer in self.layers {
                    ui.label(&layer.name);
                    ui.end_row();
                }
            })
            .response
        }).response
        
    }
}
