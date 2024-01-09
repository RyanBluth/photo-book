use eframe::egui::{Grid, Widget};

use super::layers::{Layer, Layers};

#[derive(Debug, PartialEq)]
pub struct CanvasInfo<'a> {
    pub layers: &'a mut Vec<Layer>,
}

impl<'a> Widget for CanvasInfo<'a> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> eframe::egui::Response {
        ui.allocate_ui(ui.available_size(), |ui| Layers::new(self.layers).ui(ui))
            .response
    }
}
