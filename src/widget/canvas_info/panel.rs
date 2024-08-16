use eframe::egui::{self};
use egui::InnerResponse;

use crate::{
    scene::canvas_scene::{CanvasHistoryKind, CanvasHistoryManager},
    widget::{
        canvas_info::{
            alignment::{AlignmentInfo, AlignmentInfoState},
            page_info::{PageInfo, PageInfoState},
        },
        page_canvas::CanvasState,
    },
};

use super::{
    history_info::{HistoryInfo, HistoryInfoState},
    layers::{Layer, LayerContent, Layers, LayersResponse},
    quick_layout::{QuickLayout, QuickLayoutState},
    scale_mode::{ScaleMode, ScaleModeState},
    text_control::{TextControl, TextControlState},
    transform_control::{TransformControl, TransformControlState},
};

pub struct CanvasInfoResponse {
    pub history: Option<CanvasHistoryKind>,
}

#[derive(Debug, PartialEq)]
pub struct CanvasInfo<'a> {
    pub canvas_state: &'a mut CanvasState,
    pub history_manager: &'a mut CanvasHistoryManager,
}

impl<'a> CanvasInfo<'a> {
    pub fn show(&mut self, ui: &mut egui::Ui) -> InnerResponse<CanvasInfoResponse> {
        let mut history = None;

        let response = ui.allocate_ui(ui.available_size(), |ui| {
            ui.vertical(|ui| {
                PageInfo::new(&mut PageInfoState::new(&mut self.canvas_state.page)).show(ui);

                AlignmentInfo::new(&mut AlignmentInfoState::new(
                    self.canvas_state.page.size_pixels(),
                    self.canvas_state
                        .layers
                        .iter_mut()
                        .filter(|(_, layer)| layer.selected)
                        .map(|(_, layer)| layer)
                        .collect(),
                ))
                .show(ui);

                // TODO: Handle multi select
                let selected_layer = self
                    .canvas_state
                    .layers
                    .iter_mut()
                    .filter(|x| x.1.selected)
                    .map(|(_, layer)| layer)
                    .next();

                if let Some(layer) = selected_layer {
                    if let LayerContent::TemplatePhoto {
                        region: _,
                        photo: _,
                        scale_mode,
                    } = &mut layer.content
                    {
                        ui.separator();

                        ScaleMode::new(&mut ScaleModeState::new(scale_mode)).show(ui);
                    }

                    {
                        TransformControl::new(TransformControlState::new(layer)).show(ui);

                        ui.separator();

                        if layer.content.is_text() {
                            TextControl::new(TextControlState::new(layer)).show(ui);
                            ui.separator();
                        }
                    }
                }

                QuickLayout::new(&mut QuickLayoutState::new(self.canvas_state, self.history_manager)).show(ui);

                ui.separator();

                match Layers::new(&mut self.canvas_state.layers).show(ui) {
                    LayersResponse::SelectedLayer(_) => {
                        history = Some(CanvasHistoryKind::SelectLayer)
                    }
                    LayersResponse::None => {}
                }

                if ui.button("Add Text").clicked() {
                    let layer = Layer::new_text_layer();
                    self.canvas_state.layers.insert(layer.id, layer);
                    history = Some(CanvasHistoryKind::AddText);
                }

                ui.separator();

                HistoryInfo::new(&mut HistoryInfoState::new(self.history_manager)).show(ui);
            })
        });

        InnerResponse::new(CanvasInfoResponse { history }, response.response)
    }
}
