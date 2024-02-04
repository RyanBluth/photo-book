use eframe::egui::{self, Response};
use indexmap::IndexMap;

use super::layers::{Layer, LayerId, Layers};

#[derive(Debug, PartialEq)]
pub struct CanvasInfo<'a> {
    pub layers: &'a mut IndexMap<LayerId, Layer>,
}

pub struct CanvasInfoResponse {
    pub selected_layer: Option<usize>,
    pub response: Response,
}

impl<'a> CanvasInfo<'a> {
    pub fn show(&mut self, ui: &mut egui::Ui) -> CanvasInfoResponse {
        let response = ui.allocate_ui(ui.available_size(), |ui| Layers::new(self.layers).show(ui));

        CanvasInfoResponse {
            selected_layer: response.inner.selected_layer,
            response: response.response,
        }
    }
}
