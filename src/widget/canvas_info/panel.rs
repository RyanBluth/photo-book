use eframe::egui::{self};
use egui::InnerResponse;

use crate::{
    scene::canvas_scene::{CanvasHistoryKind, CanvasHistoryManager},
    widget::{
        canvas::{CanvasState, Tool},
        canvas_info::{
            alignment::{AlignmentInfo, AlignmentInfoState},
            layers::CanvasShapeKind,
            page_info::{PageInfo, PageInfoState},
        },
    },
};

use super::{
    history_info::{HistoryInfo, HistoryInfoState},
    layers::{Layer, LayerContent, Layers, LayersResponse},
    line_edit_control::LineEditControl,
    line_tool_control::LineToolControl,
    scale_mode::{ScaleMode, ScaleModeState},
    shape_edit_control::ShapeEditControl,
    shape_tool_control::ShapeToolControl,
    text_edit_control::TextEditControl,
    text_tool_control::TextToolControl,
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

                        if matches!(layer.content, LayerContent::Text(_)) {
                            TextEditControl::new(layer).show(ui);
                            ui.separator();
                        } else if matches!(&layer.content, LayerContent::Shape(shape) if shape.kind == CanvasShapeKind::Line) {
                            LineEditControl::new(layer).show(ui);
                            ui.separator();
                        } else if matches!(layer.content, LayerContent::Shape(_)) {
                            ShapeEditControl::new(layer).show(ui);
                            ui.separator();
                        }
                    }
                } else {
                    // No layer selected - show create mode controls based on current tool
                    match self.canvas_state.current_tool {
                        Tool::Text => {
                            TextToolControl::new(&mut self.canvas_state.text_tool_settings)
                                .show(ui);
                            ui.separator();
                        }
                        Tool::Rectangle => {
                            ShapeToolControl::new(&mut self.canvas_state.rectangle_tool_settings)
                                .show(ui);
                            ui.separator();
                        }
                        Tool::Ellipse => {
                            ShapeToolControl::new(&mut self.canvas_state.ellipse_tool_settings)
                                .show(ui);
                            ui.separator();
                        }
                        Tool::Line => {
                            LineToolControl::new(&mut self.canvas_state.line_tool_settings).show(ui);
                            ui.separator();
                        }
                        Tool::Select => {
                            // No controls to show in select mode when nothing is selected
                        }
                    }
                }

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

                if ui.button("Add Shape").clicked() {
                    let layer = Layer::new_rectangle_shape_layer();
                    self.canvas_state.layers.insert(layer.id, layer);
                    history = Some(CanvasHistoryKind::AddShape);
                }

                if ui.button("Add Ellipse Shape").clicked() {
                    let layer = Layer::new_ellipse_shape_layer();
                    self.canvas_state.layers.insert(layer.id, layer);
                    history = Some(CanvasHistoryKind::AddShape);
                }

                ui.separator();

                HistoryInfo::new(&mut HistoryInfoState::new(self.history_manager)).show(ui);
            })
        });

        InnerResponse::new(CanvasInfoResponse { history }, response.response)
    }
}
