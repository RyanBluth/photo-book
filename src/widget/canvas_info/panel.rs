use eframe::egui::{self, Response};
use indexmap::IndexMap;

use super::{
    layers::{Layer, LayerId, Layers},
    transform_control::{TransformControl, TransformControlState},
};

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
        let response = ui.allocate_ui(ui.available_size(), |ui| {
            ui.vertical(|ui| {
                struct Response {
                    selected_layer_id: Option<usize>,
                }

                let mut selected_layer = self
                    .layers
                    .iter_mut()
                    .filter(|x| x.1.selected)
                    .map(|(_, layer)| layer)
                    .next();

                TransformControl::new(TransformControlState::new(&mut selected_layer)).show(ui);

                ui.separator();

                let selected_layer_id = Layers::new(self.layers).show(ui).selected_layer;

                Response { selected_layer_id }
            })
        });

        CanvasInfoResponse {
            selected_layer: response.inner.inner.selected_layer_id,
            response: response.response,
        }
    }
}
