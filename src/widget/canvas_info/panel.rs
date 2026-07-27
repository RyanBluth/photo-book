use eframe::egui::{self};
use egui::RichText;

use crate::{
    theme::color,
    widget::{
        canvas::{CanvasState, types::ToolKind},
        canvas_info::{
            alignment::{AlignmentInfo, AlignmentInfoState},
            layers::CanvasShapeKind,
        },
        edit_response::EditResponse,
        photo_adjustments::PhotoAdjustmentsState,
        photo_adjustments_panel::PhotoAdjustmentsPanel,
    },
};

use super::{
    layers::{LayerContent, Layers, LayersResponse},
    line_edit_control::LineEditControl,
    line_tool_control::LineToolControl,
    scale_mode::{ScaleMode, ScaleModeState},
    shape_edit_control::ShapeEditControl,
    shape_tool_control::ShapeToolControl,
    text_edit_control::TextEditControl,
    text_tool_control::TextToolControl,
    transform_control::{TransformControl, TransformControlState},
};

#[derive(Debug)]
pub struct CanvasArrange<'a> {
    pub canvas_state: &'a mut CanvasState,
}

#[derive(Debug)]
pub struct CanvasProperties<'a> {
    pub canvas_state: &'a mut CanvasState,
}

#[derive(Debug)]
pub struct CanvasAdjustments<'a> {
    pub canvas_state: &'a mut CanvasState,
    pub adjustments_state: &'a mut PhotoAdjustmentsState,
}

#[derive(Debug)]
pub struct CanvasLayers<'a> {
    pub canvas_state: &'a mut CanvasState,
}

impl<'a> CanvasArrange<'a> {
    pub fn show(&mut self, ui: &mut egui::Ui) -> EditResponse {
        show_scrollable_panel(ui, "canvas_arrange_scroll", egui::Margin::same(12), |ui| {
            AlignmentInfo::new(&mut AlignmentInfoState::new(
                self.canvas_state.page.size_pixels(),
                self.canvas_state.selected_layers_iter_mut().collect(),
            ))
            .show(ui)
        })
    }
}

impl<'a> CanvasProperties<'a> {
    pub fn show(&mut self, ui: &mut egui::Ui) -> EditResponse {
        show_scrollable_panel(
            ui,
            "canvas_properties_scroll",
            egui::Margin::same(12),
            |ui| self.show_context_controls(ui),
        )
    }

    fn show_context_controls(&mut self, ui: &mut egui::Ui) -> EditResponse {
        // TODO: Handle multi select
        let selected_layer = self.canvas_state.selected_layers_iter_mut().next();

        if let Some(layer) = selected_layer {
            let mut response = EditResponse::none();
            if let LayerContent::TemplatePhoto {
                region: _,
                photo: _,
                scale_mode,
            } = &mut layer.content
            {
                response |= ScaleMode::new(&mut ScaleModeState::new(scale_mode)).show(ui);
                ui.separator();
            }

            response |= TransformControl::new(TransformControlState::new(layer)).show(ui);

            if matches!(layer.content, LayerContent::Text(_)) {
                ui.separator();
                response |= TextEditControl::new(layer).show(ui);
            } else if matches!(&layer.content, LayerContent::Shape(shape) if matches!(shape.kind, CanvasShapeKind::Line { .. }))
            {
                ui.separator();
                response |= LineEditControl::new(layer).show(ui);
            } else if matches!(layer.content, LayerContent::Shape(_)) {
                ui.separator();
                response |= ShapeEditControl::new(layer).show(ui);
            }
            response
        } else {
            self.show_tool_controls(ui);
            EditResponse::none()
        }
    }

    fn show_tool_controls(&mut self, ui: &mut egui::Ui) {
        match self.canvas_state.tool_state.tool_kind() {
            ToolKind::Text => {
                TextToolControl::new(&mut self.canvas_state.text_tool_settings).show(ui);
            }
            ToolKind::Rectangle => {
                ShapeToolControl::new(&mut self.canvas_state.rectangle_tool_settings).show(ui);
            }
            ToolKind::Ellipse => {
                ShapeToolControl::new(&mut self.canvas_state.ellipse_tool_settings).show(ui);
            }
            ToolKind::Line => {
                LineToolControl::new(&mut self.canvas_state.line_tool_settings).show(ui);
            }
            ToolKind::Select => {
                self.show_empty_state(ui);
            }
        }
    }

    fn show_empty_state(&self, ui: &mut egui::Ui) {
        ui.centered_and_justified(|ui| {
            ui.label(
                RichText::new("Select a layer to edit its properties").color(color::CONTROL_TEXT),
            );
        });
    }
}

impl<'a> CanvasAdjustments<'a> {
    pub fn show(&mut self, ui: &mut egui::Ui) -> EditResponse {
        show_scrollable_panel(
            ui,
            "canvas_adjustments_scroll",
            egui::Margin::same(12),
            |ui| self.show_context_controls(ui),
        )
    }

    fn show_context_controls(&mut self, ui: &mut egui::Ui) -> EditResponse {
        let selected_photo = self
            .canvas_state
            .selected_layers_iter_mut()
            .find_map(|layer| match &mut layer.content {
                LayerContent::Photo(photo) => Some(photo),
                LayerContent::TemplatePhoto {
                    photo: Some(photo), ..
                } => Some(photo),
                _ => None,
            });

        if let Some(photo) = selected_photo {
            PhotoAdjustmentsPanel::new(&photo.photo, &mut photo.adjustments, self.adjustments_state)
                .show(ui)
        } else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new("Select a photo layer to edit its adjustments")
                        .color(color::CONTROL_TEXT),
                );
            });
            EditResponse::none()
        }
    }
}

impl<'a> CanvasLayers<'a> {
    pub fn show(&mut self, ui: &mut egui::Ui) -> EditResponse {
        show_scrollable_panel(ui, "canvas_layers_scroll", egui::Margin::ZERO, |ui| {
            match Layers::new(&mut self.canvas_state.layers).show(ui) {
                LayersResponse::Changed => EditResponse::discrete(true),
                LayersResponse::SelectedLayer(_) | LayersResponse::None => EditResponse::none(),
            }
        })
    }
}

fn show_scrollable_panel<R>(
    ui: &mut egui::Ui,
    id_salt: &'static str,
    content_margin: egui::Margin,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.allocate_ui(ui.available_size(), |ui| {
        egui::ScrollArea::vertical()
            .id_salt(id_salt)
            .max_height(ui.available_height())
            .auto_shrink([false, false])
            .content_margin(content_margin)
            .show(ui, add_contents)
            .inner
    })
    .inner
}
