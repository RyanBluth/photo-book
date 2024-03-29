use eframe::egui::{self, Response};
use egui::InnerResponse;
use indexmap::IndexMap;

use crate::{
    scene::canvas_scene::{CanvasHistoryKind, CanvasHistoryManager},
    widget::{
        canvas_info::{
            alignment::{AlignmentInfo, AlignmentInfoState},
            page_info::{PageInfo, PageInfoState},
        },
        page_canvas::Page,
    },
};

use super::{
    history_info::{HistoryInfo, HistoryInfoState},
    layers::{Layer, LayerId, Layers, LayersResponse},
    text_control::{TextControl, TextControlState},
    transform_control::{TransformControl, TransformControlState},
};

pub struct CanvasInfoResponse {
    pub history: Option<CanvasHistoryKind>,
}

#[derive(Debug, PartialEq)]
pub struct CanvasInfo<'a> {
    pub layers: &'a mut IndexMap<LayerId, Layer>,
    pub page: &'a mut Page,
    pub history_manager: &'a mut CanvasHistoryManager,
}

impl<'a> CanvasInfo<'a> {
    pub fn show(&mut self, ui: &mut egui::Ui) -> InnerResponse<CanvasInfoResponse> {
        let mut history = None;

        let response = ui.allocate_ui(ui.available_size(), |ui| {
            ui.vertical(|ui| {
                PageInfo::new(&mut PageInfoState::new(self.page)).show(ui);

                AlignmentInfo::new(&mut AlignmentInfoState::new(
                    self.page.size_pixels(),
                    self.layers
                        .iter_mut()
                        .filter(|(_, layer)| layer.selected)
                        .map(|(_, layer)| layer)
                        .collect(),
                ))
                .show(ui);

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

                match Layers::new(self.layers).show(ui) {
                    LayersResponse::SelectedLayer(_) => {
                        history = Some(CanvasHistoryKind::SelectLayer)
                    }
                    LayersResponse::None => {}
                }

                if ui.button("Add Text").clicked() {
                    let layer = Layer::new_text_layer();
                    self.layers.insert(layer.id, layer);
                    history = Some(CanvasHistoryKind::AddText);
                }

                ui.separator();

                HistoryInfo::new(&mut HistoryInfoState::new(&mut self.history_manager)).show(ui);
            })
        });

        InnerResponse::new(CanvasInfoResponse { history }, response.response)
    }
}
