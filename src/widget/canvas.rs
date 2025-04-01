
use eframe::{
    egui::{self, Context, CursorIcon, Sense, Ui},
    emath::Rot2,
    epaint::{Color32, FontId, Mesh, Pos2, Rect, Shape, Vec2},
};
use egui::{Align, Id, Layout, RichText, Stroke, StrokeKind, UiBuilder};
use indexmap::{indexmap, IndexMap};

use crate::{
    cursor_manager::CursorManager,
    debug::DebugSettings,
    dependencies::{Dependency, SingletonFor},
    id::{next_layer_id, LayerId},
    model::{edit_state::EditablePage, page::Page, scale_mode::ScaleMode},
    photo::{Photo},
    photo_manager::PhotoManager,
    project_settings::ProjectSettingsManager,
    scene::canvas_scene::{CanvasHistoryKind, CanvasHistoryManager},
    template::{Template, TemplateRegionKind},
    utils::{IdExt, RectExt, Toggle},
};

use super::{
    action_bar::{ActionBar, ActionBarResponse, ActionItem, ActionItemKind},
    auto_center::AutoCenter,
    canvas_info::{
        layers::{
            CanvasText, Layer, LayerContent, LayerTransformEditState, TextHorizontalAlignment,
            TextVerticalAlignment,
        },
        quick_layout::{self},
    },
    transformable::{
        ResizeMode, TransformHandleMode, TransformableState, TransformableWidget,
        TransformableWidgetResponse,
    },
};

pub enum CanvasResponse {
    Exit,
    EnterCropMode {
        target_layer: LayerId,
        photo: CanvasPhoto,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasPhoto {
    pub photo: Photo,
    // Normalized crop rect
    pub crop: Rect,
}

impl CanvasPhoto {
    pub fn new(photo: Photo) -> Self {
        Self {
            photo,
            crop: Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasState {
    pub layers: IndexMap<LayerId, Layer>,
    pub zoom: f32,
    pub offset: Vec2,
    pub multi_select: Option<MultiSelect>,
    pub page: EditablePage,
    pub template: Option<Template>,
    pub quick_layout_order: Vec<LayerId>,
    pub last_quick_layout: Option<quick_layout::Layout>,
    pub canvas_id: egui::Id,
    computed_initial_zoom: bool,
}

impl CanvasState {
    pub fn new() -> Self {
        Self {
            layers: IndexMap::new(),
            zoom: 1.0,
            offset: Vec2::ZERO,
            multi_select: None,
            page: EditablePage::new(Dependency::<ProjectSettingsManager>::get().with_lock(
                |manager| {
                    manager
                        .project_settings
                        .default_page
                        .clone()
                        .unwrap_or_default()
                },
            )),
            template: None,
            quick_layout_order: Vec::new(),
            last_quick_layout: None,
            canvas_id: Id::random(),
            computed_initial_zoom: false,
        }
    }

    pub fn with_layers(
        layers: IndexMap<LayerId, Layer>,
        page: EditablePage,
        template: Option<Template>,
        quick_layout_order: Vec<LayerId>,
    ) -> Self {
        Self {
            layers,
            zoom: 1.0,
            offset: Vec2::ZERO,
            multi_select: None,
            page,
            template,
            quick_layout_order: quick_layout_order,
            last_quick_layout: None,
            canvas_id: Id::random(),
            computed_initial_zoom: false,
        }
    }

    pub fn clone_with_new_widget_ids(&self) -> Self {
        let mut clone = self.clone();
        for layer in clone.layers.values_mut() {
            layer.transform_state.id = Id::random();
        }
        clone
    }

    pub fn with_photo(photo: Photo) -> Self {
        let initial_rect = match photo.max_dimension() {
            crate::photo::MaxPhotoDimension::Width => {
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 1000.0 / photo.aspect_ratio()))
            }
            crate::photo::MaxPhotoDimension::Height => {
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0 * photo.aspect_ratio(), 1000.0))
            }
        };

        let canvas_photo = CanvasPhoto::new(photo);

        let name: String = canvas_photo.photo.file_name().to_string();
        let transform_state = TransformableState {
            rect: initial_rect,
            active_handle: None,
            is_moving: false,
            handle_mode: TransformHandleMode::default(),
            rotation: 0.0,
            last_frame_rotation: 0.0,
            change_in_rotation: None,
            id: Id::random(),
        };
        let transform_edit_state = LayerTransformEditState::from(&transform_state);
        let layer = Layer {
            content: LayerContent::Photo(canvas_photo),
            name,
            visible: true,
            locked: false,
            selected: false,
            id: next_layer_id(),
            transform_edit_state,
            transform_state,
        };

        Self {
            layers: indexmap! { layer.id => layer.clone() },
            zoom: 1.0,
            offset: Vec2::ZERO,
            multi_select: None,
            page: EditablePage::new(Page::default()),
            template: None,
            quick_layout_order: vec![layer.id],
            last_quick_layout: None,
            canvas_id: Id::random(),
            computed_initial_zoom: false,
        }
    }

    pub fn with_template(template: Template) -> Self {
        // Add layer for each region in the template

        let mut layers = IndexMap::new();
        for region in &template.regions {
            let name = format!("{:?}", region.kind);
            let transform_state = TransformableState {
                rect: Rect::from_min_size(
                    Pos2::new(
                        region.relative_position.x * template.page.size().x,
                        region.relative_position.y * template.page.size().y,
                    ),
                    Vec2::new(
                        region.relative_size.x * template.page.size().x,
                        region.relative_size.y * template.page.size().y,
                    ),
                ),
                active_handle: None,
                is_moving: false,
                handle_mode: TransformHandleMode::default(),
                rotation: 0.0,
                last_frame_rotation: 0.0,
                change_in_rotation: None,
                id: Id::random(),
            };

            let transform_edit_state = LayerTransformEditState::from(&transform_state);

            match &region.kind {
                TemplateRegionKind::Image => {
                    let layer = Layer {
                        content: LayerContent::TemplatePhoto {
                            region: region.clone(),
                            photo: None,
                            scale_mode: ScaleMode::Fit,
                        },
                        name,
                        visible: true,
                        locked: false,
                        selected: false,
                        id: next_layer_id(),
                        transform_edit_state,
                        transform_state,
                    };
                    layers.insert(layer.id, layer);
                }
                TemplateRegionKind::Text {
                    sample_text,
                    font_size,
                } => {
                    let layer = Layer {
                        content: LayerContent::TemplateText {
                            region: region.clone(),
                            text: CanvasText::new(
                                sample_text.clone(),
                                *font_size,
                                FontId::default(),
                                Color32::BLACK,
                                TextHorizontalAlignment::Left,
                                TextVerticalAlignment::Top,
                            ),
                        },
                        name,
                        visible: true,
                        locked: false,
                        selected: false,
                        id: next_layer_id(),
                        transform_edit_state,
                        transform_state,
                    };

                    layers.insert(layer.id, layer);
                }
            }
        }

        let ids = layers.keys().copied().collect::<Vec<_>>();

        Self {
            layers,
            zoom: 1.0,
            offset: Vec2::ZERO,
            multi_select: None,
            page: EditablePage::new(template.page.clone()),
            template: Some(template),
            quick_layout_order: ids,
            last_quick_layout: None,
            canvas_id: Id::random(),
            computed_initial_zoom: false,
        }
    }

    pub fn swap_layer_centers_and_bounds(&mut self, layer_id1: LayerId, layer_id2: LayerId) {
        let original_child_a_rect = self
            .layers
            .get(&layer_id1)
            .unwrap()
            .transform_state
            .rect
            .clone();

        let original_child_b_rect = self
            .layers
            .get(&layer_id2)
            .unwrap()
            .transform_state
            .rect
            .clone();

        self.layers
            .get_mut(&layer_id1)
            .unwrap()
            .transform_state
            .rect = original_child_a_rect.fit_and_center_within(original_child_b_rect);

        self.layers
            .get_mut(&layer_id2)
            .unwrap()
            .transform_state
            .rect = original_child_b_rect.fit_and_center_within(original_child_a_rect);
    }

    fn is_layer_selected(&self, layer_id: &LayerId) -> bool {
        self.layers.get(layer_id).unwrap().selected
    }

    fn selected_layers_iter_mut(&mut self) -> impl Iterator<Item = &mut Layer> {
        self.layers.values_mut().filter(|layer| layer.selected)
    }

    pub fn add_photo(&mut self, photo: Photo) {
        let layer = Layer::with_photo(photo);
        self.layers.insert(layer.id, layer);
        self.update_quick_layout_order();
    }

    pub fn update_quick_layout_order(&mut self) {
        self.quick_layout_order
            .retain(|id| self.layers.contains_key(id));

        for layer in &self.layers {
            if !self.quick_layout_order.contains(&layer.0) {
                self.quick_layout_order.push(*layer.0);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MultiSelect {
    transformable_state: TransformableState,
    selected_layers: Vec<MultiSelectChild>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MultiSelectChild {
    transformable_state: TransformableState,
    id: LayerId,
}

impl MultiSelect {
    fn new(layers: &IndexMap<LayerId, Layer>) -> Self {
        let selected_ids = layers
            .iter()
            .filter(|(_, layer)| layer.selected)
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();

        let rect = Self::compute_rect(layers, &selected_ids);
        let transformable_state = TransformableState {
            rect,
            active_handle: None,
            is_moving: false,
            handle_mode: TransformHandleMode::default(),
            rotation: 0.0,
            last_frame_rotation: 0.0,
            change_in_rotation: None,
            id: Id::random(),
        };

        let res = Self {
            transformable_state: transformable_state.clone(),
            selected_layers: layers
                .iter()
                .filter(|(_, layer)| layer.selected)
                .map(|(id, transform)| MultiSelectChild {
                    transformable_state: transform
                        .transform_state
                        .to_local_space(&transformable_state),
                    id: *id,
                })
                .collect(),
        };
        res
    }

    fn update_selected<'a>(&'a mut self, layers: &'a IndexMap<LayerId, Layer>) {
        let selected_layer_ids = layers
            .iter()
            .filter(|(_, layer)| layer.selected)
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();

        let added_layers = layers
            .iter()
            .filter(|(_, layer)| layer.selected)
            .filter(|(layer_id, _)| {
                !self
                    .selected_layers
                    .iter()
                    .any(|child| child.id == **layer_id)
            })
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();

        let removed_layers = self
            .selected_layers
            .iter()
            .filter(|child| !selected_layer_ids.iter().any(|id| child.id == *id))
            .map(|child| child.id)
            .collect::<Vec<_>>();

        for layer_id in removed_layers {
            self.selected_layers.retain(|child| child.id != layer_id);
        }

        let joined_selected_ids: Vec<usize> = selected_layer_ids
            .iter()
            .chain(self.selected_layers.iter().map(|child| &child.id))
            .copied()
            .collect();

        let new_rect = Self::compute_rect(layers, &joined_selected_ids);

        self.transformable_state.rect = new_rect;

        for layer in added_layers {
            self.selected_layers.push(MultiSelectChild {
                transformable_state: layers
                    .get(&layer)
                    .unwrap()
                    .transform_state
                    .to_local_space(&self.transformable_state),
                id: layer,
            });
        }
    }
}

impl MultiSelect {
    fn compute_rect(layers: &IndexMap<LayerId, Layer>, selected_layers: &[usize]) -> Rect {
        let mut min = Vec2::splat(std::f32::MAX);
        let mut max = Vec2::splat(std::f32::MIN);

        for layer_id in selected_layers {
            let layer = &layers.get(layer_id).unwrap();

            // TODO: we should be rotating the rect here as well, but that messes things up
            let rect = layer.transform_state.rect;

            min.x = min.x.min(rect.min.x);
            min.y = min.y.min(rect.min.y);

            max.x = max.x.max(rect.max.x);
            max.y = max.y.max(rect.max.y);
        }

        Rect::from_min_max(min.to_pos2(), max.to_pos2())
    }
}

#[derive(Debug, Clone)]
enum ActionBarAction {
    SwapCenters(LayerId, LayerId),
    SwapCentersAndBounds(LayerId, LayerId),
    SwapQuickLayoutPosition(LayerId, LayerId),
    Crop(LayerId),
}

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

        // Adjust the zoom so that the page fits in the available rect
        if !self.state.computed_initial_zoom {
            let page_size = self.state.page.size_pixels() * 1.1;
            self.state.zoom = (self.available_rect.width() / page_size.x)
                .min(self.available_rect.height() / page_size.y);
            self.state.computed_initial_zoom = true;
        }

        let canvas_response = ui.allocate_rect(self.available_rect, Sense::click());
        let canvas_rect = canvas_response.rect;

        let is_pointer_on_canvas = self.is_pointer_on_canvas(ui);

        ui.set_clip_rect(canvas_rect);

        if ui.ctx().pointer_hover_pos().is_some() {
            if is_pointer_on_canvas {
                ui.input(|input| {
                    // if input.raw_scroll_delta.y != 0.0 {
                    let zoom_factor = if input.raw_scroll_delta.y > 0.0 {
                        1.1
                    } else if input.raw_scroll_delta.y < 0.0 {
                        1.0 / 1.1
                    } else {
                        1.0
                    };
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
                    self.select_photo(&layer_id, ui.ctx());
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

        // Add action bar at the bottom
        if self.state.layers.values().any(|layer| layer.selected) {
            if let Some(response) = self.show_action_bar(ui) {
                return Some(response);
            }
        }

        None
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
                    |ui: &mut Ui, _transformed_rect: Rect, transformable_state| {
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

                let transform_response: TransformableWidgetResponse<()> =
                    TransformableWidget::new(&mut transform_state).show(
                        ui,
                        available_rect,
                        self.state.zoom,
                        active && !is_preview,
                        true,
                        |ui: &mut Ui, transformed_rect: Rect, _transformable_state| {
                            Self::draw_text(
                                ui,
                                &text.text,
                                &text.font_id,
                                transformed_rect,
                                text.font_size * self.state.zoom,
                                text.color,
                                text.horizontal_alignment,
                                text.vertical_alignment,
                            );
                        },
                    );

                layer.transform_state = transform_state;
                self.state.layers.insert(*layer_id, layer.clone());

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
                    inner: (),
                    began_moving: false,
                    began_resizing: false,
                    began_rotating: false,
                    clicked: response.clicked(),
                })
            }
            LayerContent::TemplateText { region, text } => {
                let rect = Rect::from_min_max(
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

                Self::draw_text(
                    ui,
                    &text.text,
                    &text.font_id,
                    rect,
                    text.font_size * self.state.zoom,
                    text.color,
                    text.horizontal_alignment,
                    text.vertical_alignment,
                );

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
                    inner: (),
                    began_moving: false,
                    began_resizing: false,
                    began_rotating: false,
                    clicked: response.clicked(),
                })
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

    fn draw_text(
        ui: &mut Ui,
        text: &str,
        font_id: &FontId,
        rect: Rect,
        font_size: f32,
        color: Color32,
        horizontal_alignment: TextHorizontalAlignment,
        vertical_alignment: TextVerticalAlignment,
    ) {
        ui.allocate_ui_at_rect(rect, |ui| {
            ui.style_mut().interaction.selectable_labels = false;

            let layout = Layout {
                main_dir: egui::Direction::TopDown,
                main_wrap: true,
                main_align: match vertical_alignment {
                    TextVerticalAlignment::Top => Align::Min,
                    TextVerticalAlignment::Center => Align::Center,
                    TextVerticalAlignment::Bottom => Align::Max,
                },
                main_justify: true,
                cross_align: match horizontal_alignment {
                    TextHorizontalAlignment::Left => Align::Min,
                    TextHorizontalAlignment::Center => Align::Center,
                    TextHorizontalAlignment::Right => Align::Max,
                },
                cross_justify: false,
            };

            ui.with_layout(layout, |ui| {
                ui.label(
                    RichText::new(text)
                        .color(color)
                        .family(font_id.family.clone())
                        .size(font_size),
                )
            });

            // TODO: It seems like there isn't a way to rotate when drawing text with ui.label
            // The following sort of works but it makes laying out t vhe text more difficult because we can't use eguis layout system

            // let painter = ui.painter();

            // let galley: std::sync::Arc<egui::Galley> = painter.layout(
            //     text.to_string(),
            //     FontId {
            //         size: font_size,
            //         family: font_id.family.clone(),
            //     },
            //     color,
            //     rect.width(),
            // );

            // let text_pos = rect.left_center();

            // let text_shape =
            //     TextShape::new(text_pos, galley.clone(), Color32::BLACK).with_angle(angle);

            // painter.add(text_shape);
        });
    }

    fn handle_keys(&mut self, ctx: &Context) -> Option<CanvasResponse> {
        ctx.input(|input| {
            // Exit the canvas
            if input.key_pressed(egui::Key::Backspace) && input.modifiers.ctrl {
                return Some(CanvasResponse::Exit);
            }

            // Clear the selected photo
            if input.key_pressed(egui::Key::Escape) {
                self.deselect_all_photos();
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

    fn select_photo(&mut self, layer_id: &LayerId, ctx: &Context) {
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
    }

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
                .allocate_new_ui(UiBuilder::new().max_rect(bar_rect), |ui| {
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
