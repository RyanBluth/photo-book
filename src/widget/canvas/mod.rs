pub mod selection;
pub mod state;
pub mod types;
use crate::{
    model::scale_mode::ScaleMode,
    widget::{
        canvas::{
            state::TextEditMode,
            types::{ActiveTool, IdleTool, ToolState},
        },
        toolbar::ToolbarResponse,
    },
};

use eframe::{
    egui::{
        self, Align, Context, CornerRadius, CursorIcon, Id, Layout, Margin, Sense, Stroke,
        StrokeKind, Ui, UiBuilder,
    },
    emath::Rot2,
    epaint::{Color32, EllipseShape, FontId, Mesh, Pos2, Rect, RectShape, Shape, TextShape, Vec2},
};
use egui::{
    Order,
    epaint::{ColorMode, PathStroke},
};

use crate::{
    cursor_manager::CursorManager,
    debug::DebugSettings,
    dependencies::{Dependency, SingletonFor},
    id::LayerId,
    photo_manager::PhotoManager,
    scene::canvas_scene::{CanvasHistoryKind, CanvasHistoryManager},
    template::TemplateRegionKind,
    utils::{RectExt, Toggle},
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
        }
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
            ui.visuals_mut().widgets.inactive.bg_fill = egui::Color32::from_gray(30);
            ui.visuals_mut().widgets.hovered.bg_fill = egui::Color32::from_gray(40);

            egui::Frame::NONE
                .fill(egui::Color32::from_gray(25))
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

        if self.can_zoom() && ui.ctx().pointer_hover_pos().is_some() {
            if is_pointer_on_canvas {
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
        }

        let page_rect: Rect = Rect::from_center_size(
            canvas_rect.center() + self.state.offset,
            self.state.page.size_pixels() * self.state.zoom,
        );

        ui.input(|input| {
            if input.key_down(egui::Key::Space) && is_pointer_on_canvas {
                self.state.offset += input.pointer.delta();
                Dependency::<CursorManager>::get().with_lock_mut(|cursor_manager| {
                    cursor_manager.set_cursor(CursorIcon::Grabbing);
                });
                true
            } else {
                false
            }
        });

        ui.painter().rect_filled(canvas_rect, 0.0, Color32::BLACK);
        ui.painter().rect_filled(page_rect, 0.0, Color32::WHITE);

        self.draw_template(ui, page_rect);

        // Draw the layers by iterating over the layers and drawing them
        // We collect the ids into a map to avoid borrowing issues
        // TODO: Is there a better way?
        for layer_id in self.state.layers.keys().copied().collect::<Vec<LayerId>>() {
            if let Some(transform_response) = self.draw_layer(&layer_id, false, page_rect, ui) {
                let transform_state = &self.state.layers.get(&layer_id).unwrap().transform_state;

                let primary_pointer_pressed = ui.input(|input| input.pointer.primary_pressed());
                let primary_pointer_released = ui.input(|input| input.pointer.primary_released());

                // If the canvas was clicked but not on the photo then deselect the photo
                if canvas_response.clicked()
                    && !transform_state
                        .rect
                        .contains(canvas_response.interact_pointer_pos().unwrap_or(Pos2::ZERO))
                    && self.is_pointer_on_canvas(ui)
                    && self.state.is_layer_selected(&layer_id)
                {
                    self.deselect_all_photos();
                } else if transform_response.mouse_down && primary_pointer_pressed {
                    self.select_layer(&layer_id, ui.ctx());
                }

                if primary_pointer_released
                    && (transform_response.ended_moving
                        || transform_response.ended_resizing
                        || transform_response.ended_rotating)
                {
                    self.history_manager
                        .save_history(CanvasHistoryKind::Transform, self.state);
                }
            }
        }

        self.draw_multi_select(ui, page_rect);

        self.draw_tool(ui, page_rect);

        // Add action bar at the bottom
        if self.state.layers.values().any(|layer| layer.selected) {
            if let Some(response) = self.show_action_bar(ui) {
                return Some(response);
            }
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
                    let is_layer_selected = self.state.layers.values().any(|layer| layer.selected);
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
                for layer in self.state.layers.values_mut() {
                    if layer.transform_state.rect.intersects(selection_box) {
                        layer.selected = true;
                    } else {
                        layer.selected = false;
                    }
                }

                ToolState::Idle(IdleTool::Select)
            }
            ActiveTool::Text { start_pos } => {
                let relative_start_pos = self.screen_to_page_pos2(page_rect, start_pos);

                let layer = Layer::new_text_layer_with_settings(
                    &self.state.text_tool_settings,
                    Rect::from_two_pos(relative_start_pos, relative_mouse_pos),
                );
                self.state.layers.insert(layer.id, layer.clone());
                self.select_layer(&layer.id, ui.ctx());
                self.state.text_edit_mode = TextEditMode::BeginEditing(layer.id);
                ToolState::Idle(IdleTool::Select)
            }
            ActiveTool::Rectangle { start_pos } => {
                let relative_start_pos = self.screen_to_page_pos2(page_rect, start_pos);

                let layer = Layer::new_rectangle_shape_layer_with_settings(
                    &self.state.rectangle_tool_settings,
                    Rect::from_two_pos(relative_start_pos, relative_mouse_pos),
                );
                self.state.layers.insert(layer.id, layer.clone());
                self.select_layer(&layer.id, ui.ctx());
                ToolState::Idle(IdleTool::Select)
            }
            ActiveTool::Ellipse { start_pos } => {
                let relative_start_pos = self.screen_to_page_pos2(page_rect, start_pos);

                let layer = Layer::new_ellipse_shape_layer_with_settings(
                    &self.state.ellipse_tool_settings,
                    Rect::from_two_pos(relative_start_pos, relative_mouse_pos),
                );
                self.state.layers.insert(layer.id, layer.clone());
                self.select_layer(&layer.id, ui.ctx());
                ToolState::Idle(IdleTool::Select)
            }
            ActiveTool::Line { start_pos } => {
                let relative_start_pos = self.screen_to_page_pos2(page_rect, start_pos);

                let layer = Layer::new_line_shape_layer_with_settings(
                    &self.state.line_tool_settings,
                    relative_start_pos,
                    relative_mouse_pos,
                );

                self.state.layers.insert(layer.id, layer.clone());
                self.select_layer(&layer.id, ui.ctx());
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
                    Color32::from_rgba_unmultiplied(0, 0, 255, 128),
                    Stroke::new(2.0, Color32::from_rgba_unmultiplied(0, 0, 255, 128)),
                    StrokeKind::Middle,
                );
            }
            ActiveTool::Text { start_pos } => {
                let stroke = Stroke::new(DASH_LINE_STROKE, Color32::BLACK);
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

                ui.painter().line(vec![*start_pos, mouse_pos], stroke);
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

        ui.painter().rect_filled(page_rect, 0.0, Color32::WHITE);

        let current_zoom = self.state.zoom;
        self.state.zoom = zoom;

        for layer_id in self.state.layers.keys().copied().collect::<Vec<LayerId>>() {
            self.draw_layer(&layer_id, true, page_rect, ui);
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
                        ui.painter()
                            .rect_filled(region_rect, 0.0, Color32::LIGHT_BLUE);
                    }
                    TemplateRegionKind::Text {
                        sample_text: _,
                        font_size: _,
                    } => {
                        ui.painter().rect_stroke(
                            region_rect,
                            0.0,
                            Stroke::new(2.0, Color32::GRAY.gamma_multiply(0.5)),
                            StrokeKind::Outside,
                        );
                    }
                }
            }
        }
    }

    fn draw_multi_select(&mut self, ui: &mut Ui, rect: Rect) {
        let selected_layer_ids = self
            .state
            .layers
            .iter()
            .filter(|(_, layer)| layer.selected)
            .map(|(id, _)| id)
            .collect::<Vec<_>>();

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

                            // Compute the relative position of the layer in the group so we can apply transformations
                            // to each side as they are adjusted at the group level
                            // This accounts for scaling and translation
                            {
                                let delta_left =
                                    transformable_state.rect.left() - pre_transform_rect.left();
                                let delta_top =
                                    transformable_state.rect.top() - pre_transform_rect.top();
                                let delta_right =
                                    transformable_state.rect.right() - pre_transform_rect.right();
                                let delta_bottom =
                                    transformable_state.rect.bottom() - pre_transform_rect.bottom();

                                let relative_top = (pre_transform_rect.top()
                                    - layer.transform_state.rect.top())
                                .abs()
                                    / pre_transform_rect.height();

                                let relative_left = (pre_transform_rect.left()
                                    - layer.transform_state.rect.left())
                                .abs()
                                    / pre_transform_rect.width();

                                let relative_right = (pre_transform_rect.right()
                                    - layer.transform_state.rect.right())
                                .abs()
                                    / pre_transform_rect.width();

                                let relative_bottom = (pre_transform_rect.bottom()
                                    - layer.transform_state.rect.bottom())
                                .abs()
                                    / pre_transform_rect.height();

                                layer
                                    .transform_state
                                    .rect
                                    .set_left(layer.transform_state.rect.left() + delta_left);

                                layer
                                    .transform_state
                                    .rect
                                    .set_top(layer.transform_state.rect.top() + delta_top);

                                layer
                                    .transform_state
                                    .rect
                                    .set_right(layer.transform_state.rect.right() + delta_right);

                                layer
                                    .transform_state
                                    .rect
                                    .set_bottom(layer.transform_state.rect.bottom() + delta_bottom);

                                if relative_top > 0.0 {
                                    layer.transform_state.rect.set_top(
                                        transformable_state.rect.top()
                                            + relative_top * transformable_state.rect.height(),
                                    );
                                }

                                if relative_left > 0.0 {
                                    layer.transform_state.rect.set_left(
                                        transformable_state.rect.left()
                                            + relative_left * transformable_state.rect.width(),
                                    );
                                }

                                if relative_right > 0.0 {
                                    layer.transform_state.rect.set_right(
                                        transformable_state.rect.right()
                                            - relative_right * transformable_state.rect.width(),
                                    );
                                }

                                if relative_bottom > 0.0 {
                                    layer.transform_state.rect.set_bottom(
                                        transformable_state.rect.bottom()
                                            - relative_bottom * transformable_state.rect.height(),
                                    );
                                }
                            }

                            // Now rotate the layer while maintaining the relative position of the layer in the group
                            {
                                let last_frame_rotation = transformable_state.last_frame_rotation;

                                if last_frame_rotation != transformable_state.rotation {
                                    // Get the relative vec from the center of the group to the center of the layer
                                    // We can treat this a rotation of 0
                                    let layer_center_relative_to_group =
                                        layer.transform_state.rect.center().to_vec2()
                                            - transformable_state.rect.center().to_vec2();

                                    // Since we're treating the layer as if it's not rotated we can just
                                    // rotate the layer_center_relative_to_group by the change in rotation
                                    let rotation: f32 =
                                        transformable_state.rotation - last_frame_rotation;

                                    let vec_x = Vec2::new(rotation.cos(), rotation.sin());
                                    let vec_y = Vec2::new(-rotation.sin(), rotation.cos());

                                    let rotated_center = layer_center_relative_to_group.x * (vec_x)
                                        + layer_center_relative_to_group.y * (vec_y);

                                    layer.transform_state.rect.set_center(
                                        transformable_state.rect.center() + rotated_center,
                                    );

                                    layer.transform_state.rotation +=
                                        transformable_state.rotation - last_frame_rotation;
                                }
                            }
                        }
                    },
                );

                multi_select.transformable_state = transform_state;

                Some(transform_response)
            }
        } else {
            None
        };

        if let Some(transform_response) = transform_response {
            if transform_response.ended_moving
                || transform_response.ended_resizing
                || transform_response.ended_rotating
            {
                self.history_manager
                    .save_history(CanvasHistoryKind::Transform, self.state);
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
        let active = layer.selected && self.state.multi_select.is_none();

        let layer_response = match &mut layer.content {
            LayerContent::Photo(photo) => {
                let transform_response = ui
                    .push_id(
                        format!(
                            "{}_{}_CanvasPhoto_{}",
                            is_preview,
                            self.state.canvas_id.value(),
                            layer.id
                        ),
                        |ui| {
                            Dependency::<PhotoManager>::get().with_lock_mut(|photo_manager| {
                                match photo_manager
                                    .texture_for_photo_with_thumbail_backup(&photo.photo, ui.ctx())
                                { Ok(Some(texture)) => {
                                    let mut transform_state = layer.transform_state.clone();

                                    let transform_response = TransformableWidget::new(
                                    &mut transform_state,
                                )
                                .show(
                                    ui,
                                    available_rect,
                                    self.state.zoom,
                                    active && !is_preview,
                                    true,
                                    |ui: &mut Ui, transformed_rect: Rect, _transformable_state| {
                                        // If the photo is rotated swap the width and height
                                        let mesh_rect =
                                            if photo.photo.metadata.rotation().is_horizontal() {
                                                transformed_rect
                                            } else {
                                                Rect::from_center_size(
                                                    transformed_rect.center(),
                                                    Vec2::new(
                                                        transformed_rect.height(),
                                                        transformed_rect.width(),
                                                    ),
                                                )
                                            };

                                        let painter = ui.painter();
                                        let mut mesh = Mesh::with_texture(texture.id);

                                        mesh.add_rect_with_uv(mesh_rect, photo.crop, Color32::WHITE);

                                        let mesh_center: Pos2 =
                                            mesh_rect.min + Vec2::splat(0.5) * mesh_rect.size();

                                        mesh.rotate(
                                            Rot2::from_angle(
                                                photo.photo.metadata.rotation().radians(),
                                            ),
                                            mesh_center,
                                        );
                                        mesh.rotate(
                                            Rot2::from_angle(layer.transform_state.rotation),
                                            mesh_center,
                                        );

                                        painter.add(Shape::mesh(mesh));
                                    },
                                );

                                    layer.transform_state = transform_state;

                                    Some(transform_response)
                                } _ => {
                                    None
                                }}
                            })
                        },
                    )
                    .inner;

                Dependency::<DebugSettings>::get().with_lock(|debug_settings| {
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
                return transform_response;
            }
            LayerContent::Text(text) => {
                let mut transform_state = layer.transform_state.clone();

                // Check if this layer is being edited
                let is_editing = self.state.text_edit_mode.is_editing(layer_id);

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
                            if is_editing {
                                let stroke = Stroke::new(DASH_LINE_STROKE, Color32::BLACK);
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
                                    &mut text_content.text,
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
                if transform_response.double_clicked && !is_editing {
                    self.state.text_edit_mode = TextEditMode::BeginEditing(*layer_id);
                }

                // Check if we exited edit mode
                if is_editing && self.state.text_edit_mode == TextEditMode::None {
                    self.history_manager
                        .save_history(CanvasHistoryKind::EditText, self.state);
                }

                layer.transform_state = transform_state;
                let mut updated_layer = layer.clone();
                if let LayerContent::Text(text) = &mut updated_layer.content {
                    text.text = text_content.text;
                }
                self.state.layers.insert(*layer_id, updated_layer);

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
                    Dependency::<PhotoManager>::get().with_lock_mut(|photo_manager| {
                        if let Ok(Some(texture)) = photo_manager
                            .texture_for_photo_with_thumbail_backup(&photo.photo, ui.ctx())
                        {
                            let photo_size = Vec2::new(
                                photo.photo.metadata.width() as f32,
                                photo.photo.metadata.height() as f32,
                            );

                            // Rotate to match the image rotation so we can calculate the scaled rect correctly
                            let rotated_rect: Rect =
                                if photo.photo.metadata.rotation().is_horizontal()
                                    || photo.photo.metadata.rotation().radians()
                                        == std::f32::consts::PI
                                {
                                    rect
                                } else {
                                    Rect::from_center_size(
                                        rect.center(),
                                        Vec2::new(rect.height(), rect.width()),
                                    )
                                };

                            let scaled_rect = match scale_mode {
                                ScaleMode::Fit => {
                                    if photo_size.x > photo_size.y {
                                        Rect::from_center_size(
                                            rotated_rect.center(),
                                            Vec2::new(
                                                rotated_rect.width(),
                                                rotated_rect.width() / photo_size.x * photo_size.y,
                                            ),
                                        )
                                    } else {
                                        Rect::from_center_size(
                                            rotated_rect.center(),
                                            Vec2::new(
                                                rotated_rect.height() / photo_size.y * photo_size.x,
                                                rotated_rect.height(),
                                            ),
                                        )
                                    }
                                }
                                ScaleMode::Fill => {
                                    if photo_size.x > photo_size.y {
                                        Rect::from_center_size(
                                            rotated_rect.center(),
                                            Vec2::new(
                                                rotated_rect.height() / photo_size.y * photo_size.x,
                                                rotated_rect.height(),
                                            ),
                                        )
                                    } else {
                                        Rect::from_center_size(
                                            rotated_rect.center(),
                                            Vec2::new(
                                                rotated_rect.width(),
                                                rotated_rect.width() / photo_size.x * photo_size.y,
                                            ),
                                        )
                                    }
                                }
                                ScaleMode::Stretch => rotated_rect,
                            };

                            let current_clip = ui.clip_rect();

                            let clipped_rect = scaled_rect.intersect(current_clip);
                            ui.set_clip_rect(clipped_rect);

                            let painter = ui.painter();
                            let mut mesh = Mesh::with_texture(texture.id);

                            mesh.add_rect_with_uv(
                                scaled_rect.center_within(rect),
                                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2 { x: 1.0, y: 1.0 }),
                                Color32::WHITE,
                            );

                            let mesh_center: Pos2 =
                                scaled_rect.min + Vec2::splat(0.5) * scaled_rect.size();

                            mesh.rotate(
                                Rot2::from_angle(photo.photo.metadata.rotation().radians()),
                                mesh_center,
                            );

                            painter.add(Shape::mesh(mesh));

                            ui.set_clip_rect(current_clip);
                        }
                    });
                }

                if layer.selected {
                    ui.painter().rect_stroke(
                        rect,
                        0.0,
                        Stroke::new(2.0, Color32::GREEN),
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
                let is_editing = self.state.text_edit_mode.is_editing(&layer_id);

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
                    let stroke = Stroke::new(DASH_LINE_STROKE, Color32::BLACK);
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
                        &mut text_content.text,
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
                if text.text != text_content.text {
                    let mut updated_layer = layer.clone();
                    if let LayerContent::TemplateText { region: _, text } =
                        &mut updated_layer.content
                    {
                        text.text = text_content.text;
                    }
                    self.state.layers.insert(*layer_id, updated_layer);
                }

                // Double-click to enter edit mode
                if response.double_clicked() && !is_editing {
                    self.state.text_edit_mode = TextEditMode::BeginEditing(*layer_id);
                }

                // Check for exiting edit mode
                if is_editing {
                    ui.ctx().data(|data| {
                        // Check if we need to exit edit mode
                        if let Some(exit_layer_id) =
                            data.get_temp::<LayerId>(Id::new("exit_text_edit_mode"))
                        {
                            if exit_layer_id == *layer_id {
                                self.state.text_edit_mode = TextEditMode::None;
                                self.history_manager
                                    .save_history(CanvasHistoryKind::EditText, self.state);
                            }
                        }
                    });
                }

                if layer.selected {
                    ui.painter().rect_stroke(
                        rect,
                        0.0,
                        Stroke::new(2.0, Color32::GREEN),
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
                    _clicked: response.clicked(),
                    double_clicked: response.double_clicked(),
                })
            }
            LayerContent::Shape(canvas_shape) => {
                let mut transform_state = layer.transform_state.clone();
                let response = ui.push_id(
                    format!("shape_{}_{:?}", layer_id.to_string(), canvas_shape.kind),
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
                                                    .map(|(_, kind)| kind.into())
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
                                    CanvasShapeKind::Line => {
                                        // For lines, draw from actual start to end (stored in rect.min and rect.max)
                                        let rotated_rect = transformed_rect
                                            .rotate_bb_around_center(transformable_state.rotation);

                                        let start = rotated_rect.min;
                                        let end = rotated_rect.max;

                                        if let Some((stroke, _)) = canvas_shape.stroke {
                                            let zoomed_stroke = Stroke::new(
                                                stroke.width * self.state.zoom,
                                                stroke.color,
                                            );
                                            ui.painter().line_segment([start, end], zoomed_stroke);
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
        };

        return layer_response;
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
                .circle_filled(circle_rect.center(), circle_size / 2.0, Color32::RED);

            // Draw number
            ui.painter().text(
                circle_rect.center(),
                egui::Align2::CENTER_CENTER,
                (index + 1).to_string(),
                FontId::proportional(14.0),
                Color32::WHITE,
            );
        }
    }

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
    ) {
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

        let layout = Layout {
            main_dir: egui::Direction::TopDown,
            main_wrap: true,
            main_align: vertical_alignment,
            main_justify: true,
            cross_align: horizontal_alignment,
            cross_justify: false,
        };

        ui.scope_builder(UiBuilder::new().layout(layout).max_rect(rect), |ui| {
            ui.style_mut().interaction.selectable_labels = false;

            let frame = egui::Frame::new()
                .inner_margin(Margin::ZERO)
                .outer_margin(Margin::ZERO);

            frame.show(ui, |ui| {
                // Configure the text edit using the current text's properties
                let text_edit = egui::TextEdit::multiline(text)
                    .id("text-edit".into())
                    .font(FontId::new(font_size, font_id.family.clone()))
                    .text_color(color)
                    .min_size(rect.size())
                    .desired_width(rect.width())
                    .lock_focus(true)
                    .background_color(Color32::TRANSPARENT)
                    .horizontal_align(horizontal_alignment)
                    .vertical_align(vertical_alignment);

                let response = ui.add(text_edit);

                if *text_edit_mode == TextEditMode::BeginEditing(layer_id) {
                    *text_edit_mode = TextEditMode::Editing(layer_id);
                    response.request_focus();
                }

                // If user presses Enter or clicks outside, exit edit mode
                if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    *text_edit_mode = TextEditMode::None;
                }

                text_edit_mode
            });
        });
    }

    fn draw_text(
        ui: &mut Ui,
        text: &mut String,
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

        // Create the text layout with wrapping
        let wrap_width = rect.width();
        let galley = ui.fonts_mut(|f| {
            f.layout(
                text.clone(),
                FontId {
                    size: font_size,
                    family: font_id.family.clone(),
                },
                color,
                wrap_width,
            )
        });

        // For rotation, we need to think about the galley (actual text) size vs the rect size
        // The galley is positioned within the rect according to alignment
        // Then both the galley position AND the galley itself need to rotate around rect center

        // Calculate where the galley would be positioned within the unrotated rect
        let galley_size = galley.rect.size();

        // Position the galley within the rect based on alignment
        let galley_rect_in_parent = anchor.align_size_within_rect(galley_size, rect);
        let text_pos = galley_rect_in_parent.min;

        // Create text shape and apply rotation
        let mut text_shape = TextShape::new(text_pos, galley, color);

        if rotation != 0.0 {
            // Calculate the center of the bounding rect
            let rect_center = rect.center();

            // Rotate the text position around the rect center
            let rot = Rot2::from_angle(rotation);
            let offset_from_center = text_pos - rect_center;
            let rotated_offset = rot * offset_from_center;
            text_shape.pos = rect_center + rotated_offset;

            // Set the angle for the text itself
            text_shape.angle = rotation;
        }

        ui.painter().add(text_shape);
    }

    fn handle_keys(&mut self, ctx: &Context) -> Option<CanvasResponse> {
        ctx.input(|input| {
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
                } else {
                    self.deselect_all_photos();
                }
            }

            // Delete the selected photo
            if input.key_pressed(egui::Key::Delete) {
                self.state.layers.retain(|_, layer| !layer.selected);

                // Remove any layers that are in the quick layout order but are no longer in the layers map
                self.state.update_quick_layout_order();

                self.history_manager
                    .save_history(CanvasHistoryKind::DeletePhoto, self.state);
            }

            // Move the selected photo
            let mut save_transform_history = false;
            for layer in self.state.selected_layers_iter_mut() {
                // Handle movement via arrow keys
                {
                    let distance = if input.modifiers.shift { 10.0 } else { 1.0 };

                    let transform_state = &mut layer.transform_state;

                    if input.key_pressed(egui::Key::ArrowLeft) {
                        transform_state.rect =
                            transform_state.rect.translate(Vec2::new(-distance, 0.0));
                    }

                    if input.key_pressed(egui::Key::ArrowRight) {
                        transform_state.rect =
                            transform_state.rect.translate(Vec2::new(distance, 0.0));
                    }

                    if input.key_pressed(egui::Key::ArrowUp) {
                        transform_state.rect =
                            transform_state.rect.translate(Vec2::new(0.0, -distance));
                    }

                    if input.key_pressed(egui::Key::ArrowDown) {
                        transform_state.rect =
                            transform_state.rect.translate(Vec2::new(0.0, distance));
                    }

                    // Once the arrow key is released then log the history
                    if input.key_released(egui::Key::ArrowLeft)
                        || input.key_released(egui::Key::ArrowRight)
                        || input.key_released(egui::Key::ArrowUp)
                        || input.key_released(egui::Key::ArrowDown)
                    {
                        save_transform_history = true
                    }
                }
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

            for layer in self.state.selected_layers_iter_mut() {
                // Switch to scale mode
                if input.key_pressed(egui::Key::S) {
                    // TODO should the resize mode be persisted? Probably.

                    layer.transform_state.handle_mode =
                        TransformHandleMode::Resize(ResizeMode::Free);
                }

                // Switch to rotate mode
                if input.key_pressed(egui::Key::R) {
                    layer.transform_state.handle_mode = TransformHandleMode::Rotate;
                };
            }

            if save_transform_history {
                self.history_manager
                    .save_history(CanvasHistoryKind::Transform, self.state);
            }

            // Undo/Redo
            if input.key_pressed(egui::Key::Z) && input.modifiers.ctrl {
                if input.modifiers.shift {
                    self.history_manager.redo(self.state);
                } else {
                    self.history_manager.undo(self.state);
                }
            }

            None
        });

        None
    }

    fn is_pointer_on_canvas(&self, ui: &mut Ui) -> bool {
        self.available_rect.contains(
            ui.input(|input| input.pointer.hover_pos())
                .unwrap_or_default(),
        )
    }

    fn select_layer(&mut self, layer_id: &LayerId, ctx: &Context) {
        if ctx.input(|input| input.modifiers.ctrl) {
            self.state
                .layers
                .get_mut(layer_id)
                .unwrap()
                .selected
                .toggle();
        } else {
            for (_, layer) in &mut self.state.layers {
                layer.selected = layer.id == *layer_id;
            }
        }

        if self.state.is_layer_selected(layer_id) {
            self.state.tool_state = ToolState::Idle(IdleTool::Select);
        }
    }

    #[allow(dead_code)]
    fn deselect_photo(&mut self, layer_id: &LayerId) {
        self.state.layers.get_mut(layer_id).unwrap().selected = false;
        self.history_manager
            .save_history(CanvasHistoryKind::DeselectLayer, self.state);
    }

    fn deselect_all_photos(&mut self) {
        for (_, layer) in &mut self.state.layers {
            layer.selected = false;
        }
        self.history_manager
            .save_history(CanvasHistoryKind::DeselectLayer, self.state);
    }

    fn can_zoom(&self) -> bool {
        matches!(self.state.tool_state, ToolState::Idle(_))
    }

    fn show_action_bar(&mut self, ui: &mut Ui) -> Option<CanvasResponse> {
        let selected_layers: Vec<LayerId> = self
            .state
            .layers
            .iter()
            .filter(|(_, layer)| layer.selected)
            .map(|(id, _)| *id)
            .collect();

        let mut actions = vec![];

        // Add actions based on selection
        match selected_layers.len() {
            1 => {
                let layer_id = selected_layers[0];
                if let Some(layer) = self.state.layers.get(&layer_id) {
                    if let LayerContent::Photo(_photo) = &layer.content {
                        actions.push(ActionItem {
                            kind: ActionItemKind::Text("Crop".to_string()),
                            action: ActionBarAction::Crop(layer_id),
                        });
                    }
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

            match ui
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
                ActionBarResponse::Clicked(action) => {
                    match action {
                        ActionBarAction::SwapCenters(id1, id2) => {
                            let original_child_a_rect = self
                                .state
                                .layers
                                .get(&id1)
                                .unwrap()
                                .transform_state
                                .rect
                                .clone();

                            let original_child_b_rect = self
                                .state
                                .layers
                                .get(&id2)
                                .unwrap()
                                .transform_state
                                .rect
                                .clone();

                            self.state
                                .layers
                                .get_mut(&id1)
                                .unwrap()
                                .transform_state
                                .rect
                                .set_center(original_child_b_rect.center());

                            self.state
                                .layers
                                .get_mut(&id2)
                                .unwrap()
                                .transform_state
                                .rect
                                .set_center(original_child_a_rect.center());
                        }
                        ActionBarAction::SwapCentersAndBounds(id1, id2) => {
                            self.state.swap_layer_centers_and_bounds(id1, id2);
                        }
                        ActionBarAction::SwapQuickLayoutPosition(id1, id2) => {
                            if let Some(layout) = self.state.last_quick_layout {
                                let first_id_index = self
                                    .state
                                    .quick_layout_order
                                    .iter()
                                    .position(|id| *id == id1)
                                    .unwrap();

                                let second_id_index = self
                                    .state
                                    .quick_layout_order
                                    .iter()
                                    .position(|id| *id == id2)
                                    .unwrap();

                                self.state
                                    .quick_layout_order
                                    .swap(first_id_index, second_id_index);

                                layout.apply(&mut self.state, 0.0, 0.0);
                            }
                        }
                        ActionBarAction::Crop(layer_id) => {
                            if let Some(layer) = self.state.layers.get(&layer_id) {
                                if let LayerContent::Photo(photo) = &layer.content {
                                    return Some(CanvasResponse::EnterCropMode {
                                        target_layer: layer_id,
                                        photo: photo.clone(),
                                    });
                                }
                            }
                        }
                    }
                    self.history_manager
                        .save_history(CanvasHistoryKind::Transform, self.state);
                }
                _ => {}
            }
        }

        None
    }
}
