use eframe::egui::{self, Response};
use indexmap::IndexMap;

use crate::widget::{
    canvas_info::page_info::{PageInfo, PageInfoState},
    page_canvas::Page,
};

use super::{
    layers::{Layer, LayerId, Layers},
    text_control::{TextControl, TextControlState},
    transform_control::{TransformControl, TransformControlState},
};

#[derive(Debug, PartialEq)]
pub struct CanvasInfo<'a> {
    pub layers: &'a mut IndexMap<LayerId, Layer>,
    pub page: &'a mut Page,
}

pub struct CanvasInfoResponse {
    pub selected_layer: Option<usize>,
    pub response: Response,
}

impl<'a> CanvasInfo<'a> {
    pub fn show(&mut self, ui: &mut egui::Ui) -> CanvasInfoResponse {
        let response = ui.allocate_ui(ui.available_size(), |ui| {
            ui.vertical(|ui| {
                PageInfo::new(&mut PageInfoState::new(self.page)).show(ui);

                struct Response {
                    selected_layer_id: Option<usize>,
                }

                // TODO: Handle multi select
                let selected_layer = self
                    .layers
                    .iter_mut()
                    .filter(|x| x.1.selected)
                    .map(|(_, layer)| layer)
                    .next();

                if let Some(layer) = selected_layer {
                    TransformControl::new(TransformControlState::new(layer)).show(ui);

                    ui.separator();

                    if matches!(layer.content, super::layers::LayerContent::Text(_)) {
                        TextControl::new(TextControlState::new(layer)).show(ui);
                        ui.separator();
                    }
                }

                let selected_layer_id = Layers::new(self.layers).show(ui).selected_layer;

                if ui.button("Add Text").clicked() {
                    let layer = Layer::new_text_layer();
                    self.layers.insert(layer.id, layer);
                }

                Response { selected_layer_id }
            })
        });

        CanvasInfoResponse {
            selected_layer: response.inner.inner.selected_layer_id,
            response: response.response,
        }
    }
}
