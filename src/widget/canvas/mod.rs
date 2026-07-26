pub mod history;
pub mod selection;
pub mod state;
pub mod types;
use crate::{
    model::scale_mode::ScaleMode,
    utils::Vec2Ext,
    widget::{
        canvas::{
            state::TextEditMode,
            types::{ActiveTool, IdleTool, ToolState},
        },
        canvas_info::layers::LineSlope,
        toolbar::ToolbarResponse,
    },
};
use std::sync::Arc;

use eframe::{
    egui::{
        self, Align, Context, CornerRadius, CursorIcon, Id, Sense, Stroke, StrokeKind, Ui,
        UiBuilder,
    },
    emath::Rot2,
    epaint::{
        Color32, EllipseShape, FontId, Mesh, Pos2, Rect, RectShape, Shape, Tessellator, TextShape,
        Vec2,
    },
};
use egui::{
    Galley, Order,
    epaint::{ColorMode, PathStroke},
    text::LayoutJob,
};

use crate::{
    cursor_manager::CursorManager,
    debug::DebugSettings,
    dep, dep_mut,
    font_manager::FontManager,
    id::LayerId,
    photo_renderer::{PhotoRenderOptions, PhotoRenderer},
    template::TemplateRegionKind,
    theme::color,
    utils::{MeshExt, RectExt},
    widget::canvas_info::layers::{
        CanvasShapeKind, Layer, LayerContent, TextHorizontalAlignment, TextVerticalAlignment,
    },
};

use super::{
    action_bar::{ActionBar, ActionBarResponse, ActionItem, ActionItemKind},
    auto_center::AutoCenter,
    toolbar::Toolbar,
    transformable::{
        ResizeMode, TransformHandleMode, TransformableWidget, TransformableWidgetResponse,
    },
};

pub use self::{
    history::{CanvasHistoryKind, CanvasHistoryManager},
    selection::MultiSelect,
    state::CanvasState,
    types::{ActionBarAction, CanvasPhoto, CanvasResponse},
};

const DASH_SIZE: f32 = 4.0;
const DASH_LINE_STROKE: f32 = 2.0;

pub struct Canvas<'a> {
    pub state: &'a mut CanvasState,
    available_rect: Rect,
    history_manager: &'a mut CanvasHistoryManager,
    gpu_photo_adjustments: bool,
}

impl<'a> Canvas<'a> {
    pub fn new(
        state: &'a mut CanvasState,
        available_rect: Rect,
        history_manager: &'a mut CanvasHistoryManager,
    ) -> Self {
        Self {
            state,
            available_rect,
            history_manager,
            gpu_photo_adjustments: true,
        }
    }

    pub fn gpu_photo_adjustments(mut self, enabled: bool) -> Self {
        self.gpu_photo_adjustments = enabled;
        self
    }

    fn add_layer(&mut self, layer: Layer, begin_text_edit: bool) {
        let history_kind = match &layer.content {
            LayerContent::Photo(_) | LayerContent::TemplatePhoto { .. } => {
                CanvasHistoryKind::AddPhoto
            }
            LayerContent::Text(_) | LayerContent::TemplateText { .. } => CanvasHistoryKind::AddText,
            LayerContent::Shape(_) => CanvasHistoryKind::AddShape,
        };
        self.history_manager
            .apply(history_kind, self.state, |state| {
                state.add_layer(layer, begin_text_edit);
            });
    }

    pub fn show(&mut self, ui: &mut Ui) -> Option<CanvasResponse> {
        if let Some(response) = self.handle_keys(ui.ctx()) {
            return Some(response);
        }

        // Show toolbar at the top
        let toolbar_height = 40.0;
        let toolbar_rect = Rect::from_min_size(
            self.available_rect.min,
            Vec2::new(self.available_rect.width(), toolbar_height),
        );

        ui.scope_builder(UiBuilder::new().max_rect(toolbar_rect), |ui| {
            ui.visuals_mut().widgets.inactive.bg_fill = color::TOOLBAR_BACKGROUND;
            ui.visuals_mut().widgets.hovered.bg_fill = color::SURFACE;

            egui::Frame::NONE
                .fill(color::TOOLBAR_BACKGROUND)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    if let ToolbarResponse::ToolChanged(tool) =
                        Toolbar::new(self.state.tool_state.tool_kind()).show(ui)
                    {
                        self.state.tool_state = ToolState::Idle(tool.into());
                    }
                });
        });

        // Adjust available rect to account for toolbar
        let canvas_rect = Rect::from_min_size(
            self.available_rect.min + Vec2::new(0.0, toolbar_height),
            Vec2::new(
                self.available_rect.width(),
                self.available_rect.height() - toolbar_height,
            ),
        );

        // Adjust the zoom so that the page fits in the available rect
        if !self.state.computed_initial_zoom {
            let page_size = self.state.page.size_pixels() * 1.1;
            self.state.zoom =
                (canvas_rect.width() / page_size.x).min(canvas_rect.height() / page_size.y);
            self.state.computed_initial_zoom = true;
        }

        let canvas_response = ui.allocate_rect(canvas_rect, Sense::click());
        let canvas_rect = canvas_response.rect;

        let is_pointer_on_canvas = self.is_pointer_on_canvas(ui);

        ui.set_clip_rect(canvas_rect);

        if self.can_zoom() && ui.ctx().pointer_hover_pos().is_some() && is_pointer_on_canvas {
            ui.input(|input| {
                for event in &input.events {
                    if let egui::Event::MouseWheel { delta, unit, .. } = event {
                        let scroll_delta = match unit {
                            egui::MouseWheelUnit::Point => delta.y,
                            egui::MouseWheelUnit::Line => {
                                delta.y * 20.0 // Approximate line height
                            }
                            egui::MouseWheelUnit::Page => delta.y * canvas_rect.height(),
                        };

                        if scroll_delta == 0.0 {
                            continue;
                        }

                        let zoom_factor = if scroll_delta > 0.0 { 1.1 } else { 1.0 / 1.1 };

                        let new_zoom = self.state.zoom * zoom_factor;

                        if let Some(pointer_pos) = input.pointer.hover_pos() {
                            let current_page_rect: Rect = Rect::from_center_size(
                                canvas_rect.center() + self.state.offset,
                                self.state.page.size_pixels() * self.state.zoom,
                            );
                            let old_pointer_to_page = pointer_pos - current_page_rect.center();
                            let new_page_rect: Rect = Rect::from_center_size(
                                canvas_rect.center() + self.state.offset,
                                self.state.page.size_pixels() * new_zoom,
                            );
                            let new_pointer_to_page = pointer_pos - new_page_rect.center();

                            // Corrected offset calculation
                            self.state.offset += old_pointer_to_page
                                - new_pointer_to_page * (new_zoom / self.state.zoom);

                            self.state.zoom = new_zoom;
                        }
                    }
                }
            });
        }

        let page_rect: Rect = Rect::from_center_size(
            canvas_rect.center() + self.state.offset,
            self.state.page.size_pixels() * self.state.zoom,
        );

        ui.input(|input| {
            if input.key_down(egui::Key::Space) && is_pointer_on_canvas {
                self.state.offset += input.pointer.delta();
                dep_mut!(CursorManager, |cursor_manager| {
                    cursor_manager.set_cursor(CursorIcon::Grabbing);
                });
                true
            } else {
                false
            }
        });

        ui.painter().rect_filled(canvas_rect, 0.0, color::BLACK);
        ui.painter().rect_filled(page_rect, 0.0, color::WHITE);

        self.draw_template(ui, page_rect);

        // Draw the layers by iterating over the layers and drawing them
        // We collect the ids into a map to avoid borrowing issues
        // TODO: Is there a better way?
        for layer_id in self.state.layers.keys().copied().collect::<Vec<LayerId>>() {
            let (visible, locked) = self
                .state
                .layers
                .get(&layer_id)
                .map(|layer| (layer.visible, layer.locked))
                .unwrap_or((false, false));
            if !visible {
                continue;
            }

            if let Some(transform_response) = self.draw_layer(&layer_id, false, page_rect, ui) {
                let transform_state = &self.state.layers.get(&layer_id).unwrap().transform_state;

                let primary_pointer_pressed = ui.input(|input| input.pointer.primary_pressed());
                // If the canvas was clicked but not on the photo then deselect the photo
                if canvas_response.clicked()
                    && !transform_state
                        .rect
                        .contains(canvas_response.interact_pointer_pos().unwrap_or(Pos2::ZERO))
                    && self.is_pointer_on_canvas(ui)
                    && self.state.is_layer_selected(&layer_id)
                {
                    self.state.deselect_all_layers();
                } else if !locked && transform_response.mouse_down && primary_pointer_pressed {
                    let toggle = ui.input(|input| input.modifiers.ctrl);
                    self.state.select_layer(layer_id, toggle);
                }

                if transform_response.changed {
                    self.history_manager
                        .update(CanvasHistoryKind::Transform, self.state);
                }
                if transform_response.ended_transforming() {
                    self.history_manager.finish(CanvasHistoryKind::Transform);
                }
            }
        }

        self.draw_multi_select(ui, page_rect);

        self.draw_tool(ui, page_rect);

        // Add action bar at the bottom
        if self.state.selected_editable_layers_iter().next().is_some()
            && let Some(response) = self.show_action_bar(ui)
        {
            return Some(response);
        }

        None
    }

    fn draw_tool(&mut self, ui: &mut Ui, page_rect: Rect) {
        let mouse_pos = if let Some(mouse_pos) = ui.input(|input| input.pointer.interact_pos()) {
            mouse_pos
        } else {
            return;
        };

        ui.scope_builder(
            UiBuilder::new().layer_id(egui::LayerId::new(Order::Tooltip, Id::new("tool"))),
            |ui| {
                let tool_state = self.state.tool_state.clone();
                let new_tool_state = match &tool_state {
                    ToolState::Idle(tool) => self.handle_tool_idle(ui, tool, mouse_pos),
                    ToolState::Active(tool) => {
                        let primary_pointer_released =
                            ui.input(|input| input.pointer.primary_released());
                        let primary_pointer_down = ui.input(|input| input.pointer.primary_down());

                        if primary_pointer_released {
                            self.handle_tool_active_release(ui, tool, page_rect, mouse_pos)
                        } else if primary_pointer_down {
                            self.handle_tool_active_drag(ui, tool, mouse_pos)
                        } else {
                            None
                        }
                    }
                };

                if let Some(new_tool_state) = new_tool_state {
                    self.state.tool_state = new_tool_state;
                }
            },
        );
    }

    fn handle_tool_idle(
        &mut self,
        ui: &mut Ui,
        tool: &IdleTool,
        mouse_pos: Pos2,
    ) -> Option<ToolState> {
        let primary_pointer_pressed = ui.input(|input| input.pointer.primary_pressed());

        if primary_pointer_pressed && self.available_rect.contains(mouse_pos) {
            Some(match tool {
                IdleTool::Select => {
                    let is_layer_selected =
                        self.state.selected_editable_layers_iter().next().is_some();
                    if is_layer_selected {
                        return None;
                    }

                    ToolState::Active(ActiveTool::Select {
                        start_pos: mouse_pos,
                    })
                }
                IdleTool::Text => ToolState::Active(ActiveTool::Text {
                    start_pos: mouse_pos,
                }),
                IdleTool::Rectangle => ToolState::Active(ActiveTool::Rectangle {
                    start_pos: mouse_pos,
                }),
                IdleTool::Ellipse => ToolState::Active(ActiveTool::Ellipse {
                    start_pos: mouse_pos,
                }),
                IdleTool::Line => ToolState::Active(ActiveTool::Line {
                    start_pos: mouse_pos,
                }),
            })
        } else {
            None
        }
    }

    fn handle_tool_active_release(
        &mut self,
        ui: &mut Ui,
        tool: &ActiveTool,
        page_rect: Rect,
        mouse_pos: Pos2,
    ) -> Option<ToolState> {
        let relative_mouse_pos = self.screen_to_page_pos2(page_rect, &mouse_pos);

        Some(match tool {
            ActiveTool::Select { start_pos } => {
                let selection_box =
                    self.screen_to_page_rect(page_rect, Rect::from_two_pos(*start_pos, mouse_pos));
                self.state.select_layers_intersecting(selection_box);

                ToolState::Idle(IdleTool::Select)
            }
            ActiveTool::Text { start_pos } => {
                let relative_start_pos = self.screen_to_page_pos2(page_rect, start_pos);

                let layer = Layer::new_text_layer_with_settings(
                    &self.state.text_tool_settings,
                    Rect::from_two_pos(relative_start_pos, relative_mouse_pos),
                );
                self.add_layer(layer, true);
                ToolState::Idle(IdleTool::Select)
            }
            ActiveTool::Rectangle { start_pos } => {
                let relative_start_pos = self.screen_to_page_pos2(page_rect, start_pos);

                let layer = Layer::new_rectangle_shape_layer_with_settings(
                    &self.state.rectangle_tool_settings,
                    Rect::from_two_pos(relative_start_pos, relative_mouse_pos),
                );
                self.add_layer(layer, false);
                ToolState::Idle(IdleTool::Select)
            }
            ActiveTool::Ellipse { start_pos } => {
                let relative_start_pos = self.screen_to_page_pos2(page_rect, start_pos);

                let layer = Layer::new_ellipse_shape_layer_with_settings(
                    &self.state.ellipse_tool_settings,
                    Rect::from_two_pos(relative_start_pos, relative_mouse_pos),
                );
                self.add_layer(layer, false);
                ToolState::Idle(IdleTool::Select)
            }
            ActiveTool::Line { start_pos } => {
                let relative_start_pos = self.screen_to_page_pos2(page_rect, start_pos);

                let relative_end_pos = ui.input(|input| {
                    if input.modifiers.shift {
                        // Snap to 45 degrees
                        let angle = (relative_mouse_pos - relative_start_pos).angle();
                        let angle = (angle / std::f32::consts::FRAC_PI_4).round()
                            * std::f32::consts::FRAC_PI_4;
                        relative_start_pos
                            + Vec2::new(angle.cos(), angle.sin()).normalized()
                                * (relative_mouse_pos - relative_start_pos).length()
                    } else {
                        relative_mouse_pos
                    }
                });

                let (relative_start_pos, relative_end_pos) = {
                    if relative_start_pos.x > relative_end_pos.x {
                        (relative_end_pos, relative_start_pos)
                    } else {
                        (relative_start_pos, relative_end_pos)
                    }
                };

                let layer = Layer::new_line_shape_layer_with_settings(
                    &self.state.line_tool_settings,
                    relative_start_pos,
                    relative_end_pos,
                );

                self.add_layer(layer, false);
                ToolState::Idle(IdleTool::Select)
            }
        })
    }

    fn handle_tool_active_drag(
        &mut self,
        ui: &mut Ui,
        tool: &ActiveTool,
        mouse_pos: Pos2,
    ) -> Option<ToolState> {
        match tool {
            ActiveTool::Select { start_pos } => {
                let selection_box = Rect::from_two_pos(*start_pos, mouse_pos);
                ui.painter().rect(
                    selection_box,
                    0.0,
                    color::SELECTION_RECT,
                    Stroke::new(2.0, color::SELECTION_RECT),
                    StrokeKind::Middle,
                );
            }
            ActiveTool::Text { start_pos } => {
                let stroke = Stroke::new(DASH_LINE_STROKE, color::BLACK);
                let rect = Rect::from_two_pos(*start_pos, mouse_pos);
                let shape = Shape::dashed_line(
                    &[
                        rect.left_top(),
                        rect.right_top(),
                        rect.right_bottom(),
                        rect.left_bottom(),
                        rect.left_top(),
                    ],
                    stroke,
                    DASH_SIZE,
                    DASH_SIZE,
                );
                ui.painter().add(shape);
            }
            ActiveTool::Rectangle { start_pos } => {
                let scaled_stroke = self
                    .state
                    .rectangle_tool_settings
                    .stroke
                    .map(|stroke| Stroke::new(stroke.0.width * self.state.zoom, stroke.0.color))
                    .unwrap_or_default();
                let rect = Rect::from_two_pos(*start_pos, mouse_pos);
                ui.painter().rect(
                    rect,
                    0.0,
                    self.state.rectangle_tool_settings.fill_color,
                    scaled_stroke,
                    self.state
                        .rectangle_tool_settings
                        .stroke
                        .map(|stroke| stroke.1)
                        .unwrap_or(StrokeKind::Outside),
                );
            }
            ActiveTool::Ellipse { start_pos } => {
                let scaled_stroke = self
                    .state
                    .ellipse_tool_settings
                    .stroke
                    .map(|stroke| Stroke::new(stroke.0.width * self.state.zoom, stroke.0.color))
                    .unwrap_or_default();
                let rect = Rect::from_two_pos(*start_pos, mouse_pos);
                ui.painter().add(Shape::Ellipse(EllipseShape {
                    center: rect.center(),
                    radius: rect.size() / 2.0,
                    angle: 0.0,
                    fill: self.state.ellipse_tool_settings.fill_color,
                    stroke: scaled_stroke,
                }));
            }
            ActiveTool::Line { start_pos } => {
                let stroke = PathStroke {
                    width: self.state.line_tool_settings.width * self.state.zoom,
                    color: ColorMode::Solid(self.state.line_tool_settings.color),
                    ..Default::default()
                };

                let end_pos = ui.input(|input| {
                    if input.modifiers.shift {
                        // Snap to 45 degrees
                        let angle = (mouse_pos - *start_pos).angle();
                        let angle = (angle / std::f32::consts::FRAC_PI_4).round()
                            * std::f32::consts::FRAC_PI_4;
                        *start_pos
                            + Vec2::new(angle.cos(), angle.sin()).normalized()
                                * (mouse_pos - *start_pos).length()
                    } else {
                        mouse_pos
                    }
                });

                ui.painter().line(vec![*start_pos, end_pos], stroke);
            }
        }
        None
    }

    fn screen_to_page_pos2(&self, page_rect: Rect, pos: &Pos2) -> Pos2 {
        ((*pos - page_rect.min) / self.state.zoom).to_pos2()
    }

    #[allow(dead_code)]
    fn page_to_screen_pos2(&self, page_rect: Rect, pos: &Pos2) -> Pos2 {
        page_rect.min + pos.to_vec2() * self.state.zoom
    }

    fn screen_to_page_rect(&self, page_rect: Rect, rect: Rect) -> Rect {
        Rect::from_two_pos(
            self.screen_to_page_pos2(page_rect, &rect.min),
            self.screen_to_page_pos2(page_rect, &rect.max),
        )
    }

    #[allow(dead_code)]
    fn page_to_screen_rect(&self, page_rect: Rect, rect: Rect) -> Rect {
        Rect::from_two_pos(
            self.page_to_screen_pos2(page_rect, &rect.min),
            self.page_to_screen_pos2(page_rect, &rect.max),
        )
    }

    pub fn show_preview(&mut self, ui: &mut Ui, rect: Rect) {
        let zoom = (rect.width() / self.state.page.size_pixels().x)
            .min(rect.height() / self.state.page.size_pixels().y);

        let page_rect: Rect =
            Rect::from_center_size(rect.center(), self.state.page.size_pixels() * zoom);

        ui.painter().rect_filled(page_rect, 0.0, color::WHITE);

        let current_zoom = self.state.zoom;
        self.state.zoom = zoom;

        for layer_id in self.state.layers.keys().copied().collect::<Vec<LayerId>>() {
            if self
                .state
                .layers
                .get(&layer_id)
                .is_some_and(|layer| layer.visible)
            {
                self.draw_layer(&layer_id, true, page_rect, ui);
            }
        }

        self.state.zoom = current_zoom;
    }

    fn draw_template(&mut self, ui: &mut Ui, page_rect: Rect) {
        if let Some(template) = &self.state.template {
            for region in &template.regions {
                let region_rect = Rect::from_min_max(
                    page_rect.min + region.relative_position.to_vec2() * page_rect.size(),
                    page_rect.min
                        + region.relative_position.to_vec2() * page_rect.size()
                        + region.relative_size * page_rect.size(),
                );

                match &region.kind {
                    TemplateRegionKind::Image => {
                        ui.painter().rect_filled(region_rect, 0.0, color::BLUE_SOFT);
                    }
                    TemplateRegionKind::Text {
                        sample_text: _,
                        font_size: _,
                    } => {
                        ui.painter().rect_stroke(
                            region_rect,
                            0.0,
                            Stroke::new(2.0, color::SURFACE_MUTED),
                            StrokeKind::Outside,
                        );
                    }
                }
            }
        }
    }

    fn draw_multi_select(&mut self, ui: &mut Ui, rect: Rect) {
        let selected_layer_ids = self.state.selected_editable_layer_ids();

        if selected_layer_ids.len() > 1 {
            if let Some(multi_select) = &mut self.state.multi_select {
                multi_select.update_selected(&self.state.layers);
            } else {
                self.state.multi_select = Some(MultiSelect::new(&self.state.layers));
            }
        } else {
            self.state.multi_select = None;
        }

        let transform_response = if let Some(multi_select) = &mut self.state.multi_select {
            if multi_select.selected_layers.is_empty() {
                self.state.multi_select = None;
                None
            } else {
                let mut transform_state = multi_select.transformable_state.clone();

                let pre_transform_rect = transform_state.rect;

                let child_ids_content = multi_select
                    .selected_layers
                    .iter()
                    .map(|child| child.id)
                    .collect::<Vec<_>>();

                let transform_response = TransformableWidget::new(&mut transform_state).show(
                    ui,
                    rect,
                    self.state.zoom,
                    true,
                    true,
                    |_ui: &mut Ui, _transformed_rect: Rect, transformable_state| {
                        // Apply transformation to the transformable_state of each layer in the multi select
                        for child_id in child_ids_content {
                            let layer: &mut Layer = self.state.layers.get_mut(&child_id).unwrap();
                            MultiSelect::transform_child(
                                &mut layer.transform_state,
                                pre_transform_rect,
                                transformable_state,
                            );
                        }
                    },
                );

                multi_select.transformable_state = transform_state;
                multi_select.sync_children(&self.state.layers);

                Some(transform_response)
            }
        } else {
            None
        };

        if let Some(transform_response) = transform_response {
            if transform_response.changed {
                self.history_manager
                    .update(CanvasHistoryKind::Transform, self.state);
            }
            if transform_response.ended_transforming() {
                self.history_manager.finish(CanvasHistoryKind::Transform);
            }
        }
    }

    fn draw_layer(
        &mut self,
        layer_id: &LayerId,
        is_preview: bool,
        available_rect: Rect,
        ui: &mut Ui,
    ) -> Option<TransformableWidgetResponse<()>> {
        let layer = &mut self.state.layers.get_mut(layer_id).unwrap().clone();
        let editable = CanvasState::is_layer_canvas_selectable(layer);
        let active = layer.selected && editable && self.state.multi_select.is_none();

        match &mut layer.content {
            LayerContent::Photo(photo) => {
                let gpu_photo_adjustments = self.gpu_photo_adjustments && !is_preview;
                let gpu_render_key = format!(
                    "{}:{}:{}",
                    self.state.canvas_id.value(),
                    layer.id,
                    is_preview
                );
                let transform_response = ui
                    .push_id(
                        format!(
                            "{}_{}_CanvasPhoto_{}",
                            is_preview,
                            self.state.canvas_id.value(),
                            layer.id
                        ),
                        |ui| {
                            let mut transform_state = layer.transform_state.clone();
                            let transform_response = TransformableWidget::new(&mut transform_state)
                                .show(
                                    ui,
                                    available_rect,
                                    self.state.zoom,
                                    active && !is_preview,
                                    true,
                                    |ui: &mut Ui, transformed_rect: Rect, transformable_state| {
                                        let _ = PhotoRenderer::paint(
                                            ui,
                                            &photo.photo,
                                            &photo.adjustments,
                                            transformed_rect,
                                            PhotoRenderOptions::default()
                                                .gpu(gpu_photo_adjustments)
                                                .with_crop(photo.crop)
                                                .with_rotation(transformable_state.rotation)
                                                .with_render_key(&gpu_render_key),
                                        );
                                    },
                                );

                            layer.transform_state = transform_state;
                            Some(transform_response)
                        },
                    )
                    .inner;

                dep!(DebugSettings, |debug_settings| {
                    if debug_settings.show_quick_layout_order {
                        self.draw_quick_layout_number(
                            ui,
                            available_rect,
                            layer.transform_state.rect,
                            *layer_id,
                        );
                    }
                });

                self.state.layers.insert(*layer_id, layer.clone());
                transform_response
            }
            LayerContent::Text(text) => {
                let mut transform_state = layer.transform_state.clone();

                // Check if this layer is being edited
                let is_editing = editable && self.state.text_edit_mode.is_editing(layer_id);
                if !editable && self.state.text_edit_mode.is_editing(layer_id) {
                    self.state.text_edit_mode = TextEditMode::None;
                }

                // Make a mutable copy of the text content that we can modify
                let mut text_content = text.clone();

                // Get mutable reference to text edit mode so we can modify it
                let text_edit_mode = &mut self.state.text_edit_mode;

                let transform_response: TransformableWidgetResponse<()> =
                    TransformableWidget::new(&mut transform_state).show(
                        ui,
                        available_rect,
                        self.state.zoom,
                        active && !is_preview && !is_editing, // Disable transform controls when editing
                        true,
                        |ui: &mut Ui, transformed_rect: Rect, transformable_state| {
                            if is_editing && !is_preview {
                                let stroke = Stroke::new(DASH_LINE_STROKE, color::BLACK);
                                let shape = Shape::dashed_line(
                                    &[
                                        transformed_rect.left_top(),
                                        transformed_rect.right_top(),
                                        transformed_rect.right_bottom(),
                                        transformed_rect.left_bottom(),
                                        transformed_rect.left_top(),
                                    ],
                                    stroke,
                                    DASH_SIZE,
                                    DASH_SIZE,
                                );
                                ui.painter().add(shape);

                                Self::draw_editing_text(
                                    ui,
                                    &mut text_content.text,
                                    &text_content.font_id,
                                    transformed_rect,
                                    text_content.font_size * self.state.zoom,
                                    text_content.color,
                                    text_content.horizontal_alignment,
                                    text_content.vertical_alignment,
                                    layer.id,
                                    text_edit_mode,
                                );
                            } else {
                                Self::draw_text(
                                    ui,
                                    &text_content.text,
                                    &text_content.font_id,
                                    transformed_rect,
                                    text_content.font_size * self.state.zoom,
                                    text_content.color,
                                    text_content.horizontal_alignment,
                                    text_content.vertical_alignment,
                                    transformable_state.rotation,
                                );
                            }
                        },
                    );

                // Double-click to enter edit mode
                if editable && transform_response.double_clicked && !is_editing {
                    self.state.text_edit_mode = TextEditMode::BeginEditing(*layer_id);
                }

                let finished_editing =
                    is_editing && self.state.text_edit_mode == TextEditMode::None;

                layer.transform_state = transform_state;
                let mut updated_layer = layer.clone();
                if let LayerContent::Text(text) = &mut updated_layer.content {
                    text.text = text_content.text;
                }
                let text_changed = updated_layer.content != layer.content;
                self.state.layers.insert(*layer_id, updated_layer);

                if text_changed {
                    self.history_manager
                        .update(CanvasHistoryKind::EditText, self.state);
                }
                if finished_editing {
                    self.history_manager.finish(CanvasHistoryKind::EditText);
                }

                Some(transform_response)
            }
            LayerContent::TemplatePhoto {
                region,
                photo,
                scale_mode,
            } => {
                let rect: Rect = Rect::from_min_max(
                    available_rect.min + region.relative_position.to_vec2() * available_rect.size(),
                    available_rect.min
                        + region.relative_position.to_vec2() * available_rect.size()
                        + region.relative_size * available_rect.size(),
                );

                let response = ui.allocate_rect(
                    rect,
                    if is_preview {
                        Sense::focusable_noninteractive()
                    } else {
                        Sense::click()
                    },
                );

                if let Some(photo) = photo {
                    let gpu_photo_adjustments = self.gpu_photo_adjustments && !is_preview;

                    let photo_size = Vec2::new(
                        photo.photo.metadata.rotated_width() as f32,
                        photo.photo.metadata.rotated_height() as f32,
                    );

                    let scaled_rect = match scale_mode {
                        ScaleMode::Fit => {
                            if photo_size.x > photo_size.y {
                                Rect::from_center_size(
                                    rect.center(),
                                    Vec2::new(
                                        rect.width(),
                                        rect.width() / photo_size.x * photo_size.y,
                                    ),
                                )
                            } else {
                                Rect::from_center_size(
                                    rect.center(),
                                    Vec2::new(
                                        rect.height() / photo_size.y * photo_size.x,
                                        rect.height(),
                                    ),
                                )
                            }
                        }
                        ScaleMode::Fill => {
                            if photo_size.x > photo_size.y {
                                Rect::from_center_size(
                                    rect.center(),
                                    Vec2::new(
                                        rect.height() / photo_size.y * photo_size.x,
                                        rect.height(),
                                    ),
                                )
                            } else {
                                Rect::from_center_size(
                                    rect.center(),
                                    Vec2::new(
                                        rect.width(),
                                        rect.width() / photo_size.x * photo_size.y,
                                    ),
                                )
                            }
                        }
                        ScaleMode::Stretch => rect,
                    };

                    let paint_rect = scaled_rect.center_within(rect);
                    let source_uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                    let current_clip = ui.clip_rect();
                    let clipped_rect = scaled_rect.intersect(current_clip);
                    ui.set_clip_rect(clipped_rect);
                    let gpu_render_key = format!(
                        "{}:{}:{}",
                        self.state.canvas_id.value(),
                        layer.id,
                        is_preview
                    );
                    let _ = PhotoRenderer::paint(
                        ui,
                        &photo.photo,
                        &photo.adjustments,
                        paint_rect,
                        PhotoRenderOptions::default()
                            .gpu(gpu_photo_adjustments)
                            .with_clip_rect(clipped_rect)
                            .with_crop(source_uv)
                            .with_render_key(&gpu_render_key),
                    );

                    ui.set_clip_rect(current_clip);
                }

                if layer.selected {
                    ui.painter().rect_stroke(
                        rect,
                        0.0,
                        Stroke::new(2.0, color::SUCCESS),
                        StrokeKind::Outside,
                    );
                }

                Some(TransformableWidgetResponse {
                    mouse_down: response.is_pointer_button_down_on(),
                    ended_moving: false,
                    ended_resizing: false,
                    ended_rotating: false,
                    _inner: (),
                    _began_moving: false,
                    _began_resizing: false,
                    _began_rotating: false,
                    changed: false,
                    _clicked: response.clicked(),
                    double_clicked: response.double_clicked(),
                })
            }
            LayerContent::TemplateText { region, text } => {
                let rect = Rect::from_min_max(
                    available_rect.min + region.relative_position.to_vec2() * available_rect.size(),
                    available_rect.min
                        + region.relative_position.to_vec2() * available_rect.size()
                        + region.relative_size * available_rect.size(),
                );

                // Check if this layer is being edited
                let is_editing = editable && self.state.text_edit_mode.is_editing(layer_id);
                if !editable && self.state.text_edit_mode.is_editing(layer_id) {
                    self.state.text_edit_mode = TextEditMode::None;
                }

                // Create a mutable copy of the text content to work with
                let mut text_content = text.clone();

                let response = ui.allocate_rect(
                    rect,
                    if is_preview || is_editing {
                        Sense::focusable_noninteractive()
                    } else {
                        Sense::click()
                    },
                );

                if is_editing {
                    let stroke = Stroke::new(DASH_LINE_STROKE, color::BLACK);
                    let shape = Shape::dashed_line(
                        &[
                            rect.left_top(),
                            rect.right_top(),
                            rect.right_bottom(),
                            rect.left_bottom(),
                            rect.left_top(),
                        ],
                        stroke,
                        DASH_SIZE,
                        DASH_SIZE,
                    );
                    ui.painter().add(shape);

                    Self::draw_editing_text(
                        ui,
                        &mut text_content.text,
                        &text_content.font_id,
                        rect,
                        text_content.font_size * self.state.zoom,
                        text_content.color,
                        text_content.horizontal_alignment,
                        text_content.vertical_alignment,
                        layer.id,
                        &mut self.state.text_edit_mode,
                    );
                } else {
                    Self::draw_text(
                        ui,
                        &text_content.text,
                        &text_content.font_id,
                        rect,
                        text_content.font_size * self.state.zoom,
                        text_content.color,
                        text_content.horizontal_alignment,
                        text_content.vertical_alignment,
                        0.0,
                    );
                }

                // Update the layer content if the text has changed
                if editable && text.text != text_content.text {
                    let mut updated_layer = layer.clone();
                    if let LayerContent::TemplateText { region: _, text } =
                        &mut updated_layer.content
                    {
                        text.text = text_content.text;
                    }
                    self.state.layers.insert(*layer_id, updated_layer);
                    self.history_manager
                        .update(CanvasHistoryKind::EditText, self.state);
                }

                // Double-click to enter edit mode
                if editable && response.double_clicked() && !is_editing {
                    self.state.text_edit_mode = TextEditMode::BeginEditing(*layer_id);
                }

                // Check for exiting edit mode
                if is_editing && !self.state.text_edit_mode.is_editing(layer_id) {
                    self.state.text_edit_mode = TextEditMode::None;
                    self.history_manager.finish(CanvasHistoryKind::EditText);
                }

                if layer.selected {
                    ui.painter().rect_stroke(
                        rect,
                        0.0,
                        Stroke::new(2.0, color::SUCCESS),
                        StrokeKind::Outside,
                    );
                }

                // TODO: Maybe this is really just a LayerResponse?
                Some(TransformableWidgetResponse {
                    mouse_down: response.is_pointer_button_down_on(),
                    ended_moving: false,
                    ended_resizing: false,
                    ended_rotating: false,
                    _inner: (),
                    _began_moving: false,
                    _began_resizing: false,
                    _began_rotating: false,
                    changed: false,
                    _clicked: response.clicked(),
                    double_clicked: response.double_clicked(),
                })
            }
            LayerContent::Shape(canvas_shape) => {
                let mut transform_state = layer.transform_state.clone();
                let response = ui.push_id(
                    format!("shape_{}_{:?}", layer_id, canvas_shape.kind),
                    |ui| {
                        TransformableWidget::new(&mut transform_state).show(
                            ui,
                            available_rect,
                            self.state.zoom,
                            active && !is_preview,
                            true,
                            |ui: &mut Ui, transformed_rect: Rect, transformable_state| {
                                match canvas_shape.kind {
                                    CanvasShapeKind::Rectangle { corner_radius } => {
                                        let rotation = transformable_state.rotation;
                                        let shape = Shape::Rect(
                                            RectShape::new(
                                                transformed_rect,
                                                CornerRadius::same(corner_radius as u8),
                                                canvas_shape.fill_color,
                                                canvas_shape
                                                    .stroke
                                                    .map(|(stroke, _)| {
                                                        Stroke::new(
                                                            stroke.width * self.state.zoom,
                                                            stroke.color,
                                                        )
                                                    })
                                                    .unwrap_or(Stroke::NONE),
                                                canvas_shape
                                                    .stroke
                                                    .map(|(_, kind)| kind)
                                                    .unwrap_or(StrokeKind::Outside),
                                            )
                                            .with_angle_and_pivot(
                                                rotation,
                                                transformed_rect.center(),
                                            ),
                                        );
                                        ui.painter().add(shape);
                                    }
                                    CanvasShapeKind::Ellipse => {
                                        let rotation = transformable_state.rotation;
                                        let shape = Shape::Ellipse(EllipseShape {
                                            center: transformed_rect.center(),
                                            radius: Vec2::new(
                                                transformed_rect.width() / 2.0,
                                                transformed_rect.height() / 2.0,
                                            ),
                                            fill: canvas_shape.fill_color,
                                            stroke: canvas_shape
                                                .stroke
                                                .map(|(stroke, _)| {
                                                    Stroke::new(
                                                        stroke.width * self.state.zoom,
                                                        stroke.color,
                                                    )
                                                })
                                                .unwrap_or(Stroke::NONE),
                                            angle: rotation,
                                        });
                                        ui.painter().add(shape);
                                    }
                                    CanvasShapeKind::Line { ref slope } => {
                                        if let Some((stroke, _)) = canvas_shape.stroke {
                                            let rotation = transformable_state.rotation;
                                            let zoomed_stroke = Stroke::new(
                                                stroke.width * self.state.zoom,
                                                stroke.color,
                                            );
                                            let (rotated_start, rotated_end) = match slope {
                                                LineSlope::Positive => (
                                                    transformed_rect
                                                        .left_bottom()
                                                        .to_vec2()
                                                        .rotate_around(
                                                            transformed_rect.center().to_vec2(),
                                                            rotation,
                                                        ),
                                                    transformed_rect
                                                        .right_top()
                                                        .to_vec2()
                                                        .rotate_around(
                                                            transformed_rect.center().to_vec2(),
                                                            rotation,
                                                        ),
                                                ),
                                                LineSlope::Negative => (
                                                    transformed_rect
                                                        .left_top()
                                                        .to_vec2()
                                                        .rotate_around(
                                                            transformed_rect.center().to_vec2(),
                                                            rotation,
                                                        ),
                                                    transformed_rect
                                                        .right_bottom()
                                                        .to_vec2()
                                                        .rotate_around(
                                                            transformed_rect.center().to_vec2(),
                                                            rotation,
                                                        ),
                                                ),
                                            };
                                            ui.painter().line_segment(
                                                [rotated_start.to_pos2(), rotated_end.to_pos2()],
                                                zoomed_stroke,
                                            );
                                        }
                                    }
                                }
                            },
                        )
                    },
                );
                let mut updated_layer = layer.clone();
                updated_layer.transform_state = transform_state;
                self.state.layers.insert(*layer_id, updated_layer);
                Some(response.inner)
            }
        }
    }

    fn draw_quick_layout_number(
        &self,
        ui: &mut Ui,
        available_rect: Rect,
        rect: Rect,
        layer_id: LayerId,
    ) {
        // Find index of layer_id in quick_layout_order
        if let Some(index) = self
            .state
            .quick_layout_order
            .iter()
            .position(|id| *id == layer_id)
        {
            let circle_pos =
                available_rect.left_top() + (rect.left_top() * self.state.zoom).to_vec2();

            let circle_size = 240.0 * self.state.zoom;
            let circle_rect = Rect::from_min_size(circle_pos, Vec2::splat(circle_size));
            //circle_rect = circle_rect.translate(self.state.offset);

            // Draw circle background
            ui.painter()
                .circle_filled(circle_rect.center(), circle_size / 2.0, color::ERROR);

            // Draw number
            ui.painter().text(
                circle_rect.center(),
                egui::Align2::CENTER_CENTER,
                (index + 1).to_string(),
                FontId::proportional(14.0),
                color::WHITE,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_editing_text(
        ui: &mut Ui,
        text: &mut String,
        font_id: &FontId,
        rect: Rect,
        font_size: f32,
        color: Color32,
        horizontal_alignment: TextHorizontalAlignment,
        vertical_alignment: TextVerticalAlignment,
        layer_id: LayerId,
        text_edit_mode: &mut TextEditMode,
    ) -> egui::Response {
        let horizontal_alignment = match horizontal_alignment {
            TextHorizontalAlignment::Left => Align::Min,
            TextHorizontalAlignment::Center => Align::Center,
            TextHorizontalAlignment::Right => Align::Max,
        };

        let vertical_alignment = match vertical_alignment {
            TextVerticalAlignment::Top => Align::Min,
            TextVerticalAlignment::Center => Align::Center,
            TextVerticalAlignment::Bottom => Align::Max,
        };

        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            ui.style_mut().interaction.selectable_labels = false;

            // `TextEdit::min_size` only constrains width in egui 0.35. Use
            // `add_sized` so the editing surface remains bounded by the layer
            // rectangle and vertical alignment uses the same bounds as painting.
            let text_edit_font_id = FontManager::available_font_id(ui.ctx(), font_id, font_size);
            let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                Self::layout_text(
                    ui,
                    text.as_str(),
                    text_edit_font_id.clone(),
                    color,
                    wrap_width,
                    horizontal_alignment,
                )
            };
            let text_edit = egui::TextEdit::multiline(text)
                .id("text-edit".into())
                .font(text_edit_font_id.clone())
                .text_color(color)
                .desired_width(rect.width())
                .lock_focus(true)
                .frame(egui::Frame::NONE)
                .layouter(&mut layouter)
                .horizontal_align(horizontal_alignment)
                .vertical_align(vertical_alignment);

            let response = ui.add_sized(rect.size(), text_edit);

            if *text_edit_mode == TextEditMode::BeginEditing(layer_id) {
                *text_edit_mode = TextEditMode::Editing(layer_id);
                response.request_focus();
            }

            if response.lost_focus() {
                *text_edit_mode = TextEditMode::None;
            }

            response
        })
        .inner
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_text(
        ui: &mut Ui,
        text: &str,
        font_id: &FontId,
        rect: Rect,
        font_size: f32,
        color: Color32,
        horizontal_alignment: TextHorizontalAlignment,
        vertical_alignment: TextVerticalAlignment,
        rotation: f32,
    ) {
        let horizontal_align = match horizontal_alignment {
            TextHorizontalAlignment::Left => Align::Min,
            TextHorizontalAlignment::Center => Align::Center,
            TextHorizontalAlignment::Right => Align::Max,
        };

        let vertical_align = match vertical_alignment {
            TextVerticalAlignment::Top => Align::Min,
            TextVerticalAlignment::Center => Align::Center,
            TextVerticalAlignment::Bottom => Align::Max,
        };

        let anchor = egui::Align2([horizontal_align, vertical_align]);

        let layout_font_id = FontManager::available_font_id(ui.ctx(), font_id, font_size);
        let galley = Self::layout_text(
            ui,
            text,
            layout_font_id,
            color,
            rect.width(),
            horizontal_align,
        );

        // Match TextEdit's overflow behavior: once text is larger than the
        // layer, pin it to the layer's leading edge instead of centering the
        // oversized galley outside the edit bounds.
        let text_pos = Self::text_origin_in_rect(&galley, rect, anchor);

        if rotation == 0.0 {
            let text_shape = TextShape::new(text_pos, galley, color);
            ui.painter().with_clip_rect(rect).add(text_shape);
        } else if rect.contains_rect(galley.mesh_bounds.translate(text_pos.to_vec2())) {
            // Avoid custom tessellation when no glyph geometry needs clipping.
            let rotation_transform = Rot2::from_angle(rotation);
            let mut text_shape = TextShape::new(text_pos, galley, color);
            text_shape.pos = rect.center() + rotation_transform * (text_pos - rect.center());
            text_shape.angle = rotation;
            ui.painter().add(text_shape);
        } else {
            // egui clip rectangles are axis-aligned. Clip the tessellated text
            // in local layer space, then rotate the already-clipped mesh.
            let mesh =
                Self::rotated_clipped_text_mesh(ui, &galley, text_pos, rect, rotation, color);
            ui.painter().add(Shape::mesh(mesh));
        }
    }

    fn layout_text(
        ui: &Ui,
        text: &str,
        font_id: FontId,
        color: Color32,
        wrap_width: f32,
        horizontal_alignment: Align,
    ) -> Arc<Galley> {
        let mut layout_job = LayoutJob::simple(text.to_owned(), font_id, color, wrap_width);
        layout_job.halign = horizontal_alignment;
        layout_job.keep_trailing_whitespace = true;
        ui.fonts_mut(|fonts| fonts.layout_job(layout_job))
    }

    fn text_origin_in_rect(galley: &Galley, rect: Rect, anchor: egui::Align2) -> Pos2 {
        let aligned_min = anchor
            .align_size_within_rect(galley.size(), rect)
            .intersect(rect)
            .min;

        aligned_min - Vec2::new(galley.rect.left(), 0.0)
    }

    fn rotated_clipped_text_mesh(
        ui: &Ui,
        galley: &Arc<Galley>,
        text_pos: Pos2,
        clip_rect: Rect,
        rotation: f32,
        color: Color32,
    ) -> Mesh {
        let tessellation_options = ui.ctx().tessellation_options(Clone::clone);
        let font_image_size = ui.fonts(|fonts| fonts.font_image_size());
        let mut tessellator = Tessellator::new(
            ui.ctx().pixels_per_point(),
            tessellation_options,
            font_image_size,
            Vec::new(),
        );
        let mut text_mesh = Mesh::default();
        tessellator.tessellate_text(
            &TextShape::new(text_pos, Arc::clone(galley), color),
            &mut text_mesh,
        );

        let mut clipped_mesh = text_mesh.clip_mesh(clip_rect);
        clipped_mesh.rotate(Rot2::from_angle(rotation), clip_rect.center());
        clipped_mesh
    }

    fn handle_keys(&mut self, ctx: &Context) -> Option<CanvasResponse> {
        ctx.input_mut(|input| {
            // Exit the canvas
            if input.key_pressed(egui::Key::Backspace) && input.modifiers.ctrl {
                return Some(CanvasResponse::Exit);
            }

            // Clear the selected photo or exit text edit mode
            if input.key_pressed(egui::Key::Escape) {
                if matches!(
                    self.state.text_edit_mode,
                    TextEditMode::Editing(_) | TextEditMode::BeginEditing(_)
                ) {
                    self.state.text_edit_mode = TextEditMode::None;
                    self.history_manager.finish(CanvasHistoryKind::EditText);
                } else {
                    self.state.deselect_all_layers();
                }
            }

            // Delete the selected layers
            if input.key_pressed(egui::Key::Delete) {
                self.history_manager.apply(
                    CanvasHistoryKind::DeleteLayers,
                    self.state,
                    CanvasState::delete_selected_editable_layers,
                );
            }

            let distance = if input.modifiers.shift { 10.0 } else { 1.0 };
            let mut nudge = Vec2::ZERO;
            if input.key_pressed(egui::Key::ArrowLeft) {
                nudge.x -= distance;
            }
            if input.key_pressed(egui::Key::ArrowRight) {
                nudge.x += distance;
            }
            if input.key_pressed(egui::Key::ArrowUp) {
                nudge.y -= distance;
            }
            if input.key_pressed(egui::Key::ArrowDown) {
                nudge.y += distance;
            }
            let arrow_released = input.key_released(egui::Key::ArrowLeft)
                || input.key_released(egui::Key::ArrowRight)
                || input.key_released(egui::Key::ArrowUp)
                || input.key_released(egui::Key::ArrowDown);
            let arrow_still_down = input.key_down(egui::Key::ArrowLeft)
                || input.key_down(egui::Key::ArrowRight)
                || input.key_down(egui::Key::ArrowUp)
                || input.key_down(egui::Key::ArrowDown);
            if nudge != Vec2::ZERO {
                self.state.nudge_selected_layers(nudge);
                self.history_manager
                    .update(CanvasHistoryKind::Transform, self.state);
            }
            if arrow_released && !arrow_still_down {
                self.history_manager.finish(CanvasHistoryKind::Transform);
            }

            if self.state.tool_state.is_idle() {
                if input.key_pressed(egui::Key::V) {
                    self.state.tool_state = ToolState::Idle(IdleTool::Select);
                }
                if input.key_pressed(egui::Key::T) {
                    self.state.tool_state = ToolState::Idle(IdleTool::Text);
                }
                if input.key_pressed(egui::Key::U) {
                    self.state.tool_state = ToolState::Idle(IdleTool::Rectangle);
                }
                if input.key_pressed(egui::Key::O) {
                    self.state.tool_state = ToolState::Idle(IdleTool::Ellipse);
                }
                if input.key_pressed(egui::Key::L) {
                    self.state.tool_state = ToolState::Idle(IdleTool::Line);
                }
            }

            if input.key_pressed(egui::Key::S) {
                self.state
                    .set_selected_handle_mode(TransformHandleMode::Resize(ResizeMode::Free));
            }
            if input.key_pressed(egui::Key::R) {
                self.state
                    .set_selected_handle_mode(TransformHandleMode::Rotate);
            }

            // Match the modifiers carried by the key event. Besides consuming
            // the shortcut, this works with all egui input sources rather than
            // depending on the window backend's global modifier state.
            let redo_shortcut = egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Z,
            );
            let undo_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z);
            if input.consume_shortcut(&redo_shortcut) {
                self.history_manager.redo(self.state);
            } else if input.consume_shortcut(&undo_shortcut) {
                self.history_manager.undo(self.state);
            }

            None
        })
    }

    fn is_pointer_on_canvas(&self, ui: &mut Ui) -> bool {
        self.available_rect.contains(
            ui.input(|input| input.pointer.hover_pos())
                .unwrap_or_default(),
        )
    }

    fn can_zoom(&self) -> bool {
        matches!(self.state.tool_state, ToolState::Idle(_))
    }

    fn show_action_bar(&mut self, ui: &mut Ui) -> Option<CanvasResponse> {
        let selected_layers = self.state.selected_editable_layer_ids();

        let mut actions = vec![];

        // Add actions based on selection
        match selected_layers.len() {
            1 => {
                let layer_id = selected_layers[0];
                if let Some(layer) = self.state.layers.get(&layer_id)
                    && let LayerContent::Photo(_photo) = &layer.content
                {
                    actions.push(ActionItem {
                        kind: ActionItemKind::Text("Crop".to_string()),
                        action: ActionBarAction::Crop(layer_id),
                    });
                }
            }
            2 => {
                actions.extend_from_slice(&[
                    ActionItem {
                        kind: ActionItemKind::Text("Swap Centers".to_string()),
                        action: ActionBarAction::SwapCenters(
                            selected_layers[0],
                            selected_layers[1],
                        ),
                    },
                    ActionItem {
                        kind: ActionItemKind::Text("Swap Centers and Bounds".to_string()),
                        action: ActionBarAction::SwapCentersAndBounds(
                            selected_layers[0],
                            selected_layers[1],
                        ),
                    },
                    ActionItem {
                        kind: ActionItemKind::Text("Swap Quick Layout Position".to_string()),
                        action: ActionBarAction::SwapQuickLayoutPosition(
                            selected_layers[0],
                            selected_layers[1],
                        ),
                    },
                ]);
            }
            _ => {}
        }
        if !actions.is_empty() {
            let bar_height = 40.0;
            let bar_margin_bottom: f32 = 40.0;

            let bar_rect = Rect::from_min_size(
                Pos2::new(
                    self.available_rect.left(),
                    self.available_rect.max.y - bar_margin_bottom - bar_height / 2.0,
                ),
                Vec2::new(self.available_rect.width(), bar_height),
            );

            let action_bar_id: String = actions
                .iter()
                .map(|item| format!("{:?}", item.action))
                .collect::<String>();

            if let ActionBarResponse::Clicked(action) = ui
                .scope_builder(UiBuilder::new().max_rect(bar_rect), |ui| {
                    AutoCenter::new(format!("action_bar_{}", action_bar_id))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| ActionBar::with_items(actions).show(ui))
                                .inner
                        })
                        .inner
                })
                .inner
            {
                match action {
                    ActionBarAction::SwapCenters(id1, id2) => {
                        self.history_manager.apply(
                            CanvasHistoryKind::Transform,
                            self.state,
                            |state| state.swap_layer_centers(id1, id2),
                        );
                    }
                    ActionBarAction::SwapCentersAndBounds(id1, id2) => {
                        self.history_manager.apply(
                            CanvasHistoryKind::Transform,
                            self.state,
                            |state| state.swap_layer_centers_and_bounds(id1, id2),
                        );
                    }
                    ActionBarAction::SwapQuickLayoutPosition(id1, id2) => {
                        self.history_manager.apply(
                            CanvasHistoryKind::Transform,
                            self.state,
                            |state| state.swap_quick_layout_position(id1, id2),
                        );
                    }
                    ActionBarAction::Crop(layer_id) => {
                        if let Some(layer) = self.state.editable_layer(&layer_id)
                            && matches!(layer.content, LayerContent::Photo(_))
                        {
                            return Some(CanvasResponse::EnterCropMode {
                                target_layer: layer_id,
                            });
                        }
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;

    struct MultiSelectRotationHarnessState {
        canvas: CanvasState,
        history: CanvasHistoryManager,
        canvas_origin: Pos2,
    }

    fn rotate_multi_select_by(
        harness: &mut Harness<'_, MultiSelectRotationHarnessState>,
        angle: f32,
    ) {
        let state = harness.state();
        let transform = &state
            .canvas
            .multi_select
            .as_ref()
            .unwrap()
            .transformable_state;
        let origin = state.canvas_origin.to_vec2();
        let center = transform.rect.center() + origin;
        let start = transform.rect.rotated_corners(transform.rotation)[3] + origin;
        let end = center + Rot2::from_angle(angle) * (start - center);

        harness.hover_at(start);
        harness.run();
        harness.drag_at(start);
        harness.run();
        harness.hover_at(end);
        harness.run();
        harness.event(egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }

    #[test]
    fn editing_text_box_height_matches_layer_height() {
        let mut harness = Harness::new_ui(|ui| {
            let size = Vec2::new(180.0, 120.0);
            let rect = Rect::from_min_size(ui.min_rect().min + Vec2::new(25.0, 35.0), size);
            let mut text = "Centered text".to_owned();
            let mut edit_mode = TextEditMode::Editing(1);

            let response = Canvas::draw_editing_text(
                ui,
                &mut text,
                &FontId::proportional(16.0),
                rect,
                16.0,
                Color32::BLACK,
                TextHorizontalAlignment::Center,
                TextVerticalAlignment::Center,
                1,
                &mut edit_mode,
            );

            assert_eq!(response.rect, rect);
        });

        harness.run();
    }

    #[test]
    fn centered_text_centers_each_line() {
        let mut harness = Harness::new_ui(|ui| {
            let galley = Canvas::layout_text(
                ui,
                "A much longer line\nshort",
                FontId::proportional(16.0),
                Color32::BLACK,
                300.0,
                Align::Center,
            );

            assert_eq!(galley.rows.len(), 2);
            let first_center = galley.rows[0].rect().center().x;
            let second_center = galley.rows[1].rect().center().x;
            assert!((first_center - second_center).abs() < 1.01);
        });

        harness.run();
    }

    #[test]
    fn oversized_centered_text_starts_at_bounds_top() {
        let mut harness = Harness::new_ui(|ui| {
            let galley = Canvas::layout_text(
                ui,
                "one\ntwo\nthree\nfour",
                FontId::proportional(16.0),
                Color32::BLACK,
                180.0,
                Align::Center,
            );
            let rect = Rect::from_min_size(Pos2::new(25.0, 35.0), Vec2::new(180.0, 30.0));

            assert!(galley.size().y > rect.height());
            let origin = Canvas::text_origin_in_rect(&galley, rect, egui::Align2::CENTER_CENTER);
            assert_eq!(origin.y, rect.top());
        });

        harness.run();
    }

    #[test]
    fn rotated_text_mesh_stays_inside_rotated_bounds() {
        let mut harness = Harness::new_ui(|ui| {
            let rect = Rect::from_min_size(Pos2::new(25.0, 35.0), Vec2::new(100.0, 30.0));
            let galley = Canvas::layout_text(
                ui,
                "one\ntwo\nthree\nfour",
                FontId::proportional(16.0),
                Color32::BLACK,
                rect.width(),
                Align::Center,
            );
            let origin = Canvas::text_origin_in_rect(&galley, rect, egui::Align2::CENTER_CENTER);
            let rotation = std::f32::consts::FRAC_PI_4;
            let mesh = Canvas::rotated_clipped_text_mesh(
                ui,
                &galley,
                origin,
                rect,
                rotation,
                Color32::BLACK,
            );
            let inverse_rotation = Rot2::from_angle(-rotation);
            let expanded_rect = rect.expand(0.01);

            assert!(!mesh.is_empty());
            assert!(mesh.vertices.iter().all(|vertex| {
                let unrotated = rect.center() + inverse_rotation * (vertex.pos - rect.center());
                expanded_rect.contains(unrotated)
            }));
        });

        harness.run();
    }

    #[test]
    fn consecutive_multi_select_rotations_can_each_be_undone() {
        let mut canvas = CanvasState::new();
        canvas.zoom = 1.0;
        let mut first = Layer::new_rectangle_shape_layer();
        let first_id = first.id;
        first.transform_state.rect =
            Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(150.0, 150.0));
        let mut second = Layer::new_rectangle_shape_layer();
        let second_id = second.id;
        second.transform_state.rect =
            Rect::from_min_max(Pos2::new(200.0, 200.0), Pos2::new(250.0, 250.0));
        canvas.layers.insert(first_id, first);
        canvas.layers.insert(second_id, second);
        let history = CanvasHistoryManager::with_initial_state(canvas.clone());
        canvas.layers[&first_id].selected = true;
        canvas.layers[&second_id].selected = true;

        let mut harness = Harness::new_ui_state(
            |ui, state: &mut MultiSelectRotationHarnessState| {
                state.canvas_origin = ui.max_rect().min;
                let MultiSelectRotationHarnessState {
                    canvas, history, ..
                } = state;
                Canvas::new(canvas, ui.max_rect(), history).draw_multi_select(ui, ui.max_rect());
            },
            MultiSelectRotationHarnessState {
                canvas,
                history,
                canvas_origin: Pos2::ZERO,
            },
        );
        harness
            .state_mut()
            .canvas
            .multi_select
            .as_mut()
            .unwrap()
            .transformable_state
            .handle_mode = TransformHandleMode::Rotate;

        rotate_multi_select_by(&mut harness, 0.2);
        let first_rotation = harness.state().canvas.layers[&first_id]
            .transform_state
            .rotation;
        let first_group_rotation = harness
            .state()
            .canvas
            .multi_select
            .as_ref()
            .unwrap()
            .transformable_state
            .rotation;
        rotate_multi_select_by(&mut harness, 0.2);

        assert_eq!(harness.state().history.history_len(), 2);

        let state = harness.state_mut();
        state.history.undo(&mut state.canvas);
        assert!(
            (state.canvas.layers[&first_id].transform_state.rotation - first_rotation).abs()
                < f32::EPSILON
        );
        assert!(state.canvas.layers[&first_id].selected);
        assert!(state.canvas.layers[&second_id].selected);
        assert!(
            (state
                .canvas
                .multi_select
                .as_ref()
                .unwrap()
                .transformable_state
                .rotation
                - first_group_rotation)
                .abs()
                < f32::EPSILON
        );

        state.history.undo(&mut state.canvas);
        assert!(
            state.canvas.layers[&first_id]
                .transform_state
                .rotation
                .abs()
                < f32::EPSILON
        );
        assert!(
            state.canvas.layers[&second_id]
                .transform_state
                .rotation
                .abs()
                < f32::EPSILON
        );
        assert!(state.canvas.layers[&first_id].selected);
        assert!(state.canvas.layers[&second_id].selected);
        assert!(state.canvas.multi_select.is_some());
        assert!(matches!(
            state
                .canvas
                .multi_select
                .as_ref()
                .unwrap()
                .transformable_state
                .handle_mode,
            TransformHandleMode::Rotate
        ));
    }
}
