use std::{fmt::Display, str::FromStr};

use eframe::{
    egui::{self, Context, CursorIcon, Sense, Ui},
    emath::Rot2,
    epaint::{Color32, FontId, Mesh, Pos2, Rect, Shape, Vec2},
};
use egui::{ecolor::ParseHexColorError, epaint::TextShape, layers, Id};
use indexmap::{indexmap, IndexMap};
use strum_macros::EnumIter;

use crate::{
    cursor_manager::CursorManager,
    dependencies::{Dependency, Singleton, SingletonFor},
    photo::Photo,
    photo_manager::PhotoManager,
    scene::canvas_scene::{CanvasHistoryKind, CanvasHistoryManager},
    utils::{IdExt, RectExt, Toggle},
};

use super::{
    canvas_info::layers::{
        next_layer_id, CanvasTextAlignment, EditableValue, Layer, LayerContent, LayerId,
        LayerTransformEditState,
    },
    image_gallery::ImageGalleryState,
    transformable::{
        ResizeMode, TransformHandleMode, TransformableState, TransformableWidget,
        TransformableWidgetResponse,
    },
};

pub enum CanvasResponse {
    Exit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasPhoto {
    pub photo: Photo,
}

impl CanvasPhoto {
    pub fn new(photo: Photo) -> Self {
        Self { photo }
    }
}

#[derive(Debug, Clone, PartialEq, Copy, EnumIter)]
pub enum Unit {
    Pixels,
    Inches,
    Centimeters,
}

impl Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unit::Pixels => write!(f, "Pixels"),
            Unit::Inches => write!(f, "Inches"),
            Unit::Centimeters => write!(f, "Centimeters"),
        }
    }
}

impl FromStr for Unit {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Pixels" => Ok(Unit::Pixels),
            "Inches" => Ok(Unit::Inches),
            "Centimeters" => Ok(Unit::Centimeters),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageEditState {
    pub width: EditableValue<f32>,
    pub height: EditableValue<f32>,
    pub ppi: EditableValue<i32>,
    pub unit: EditableValue<Unit>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    size: Vec2,
    ppi: i32,
    unit: Unit,

    pub edit_state: PageEditState,
}

impl Page {
    fn a4() -> Self {
        let ppi = 300;
        let unit = Unit::Inches;

        Self {
            size: Vec2::new(8.27, 11.69),
            ppi,
            unit,
            edit_state: PageEditState {
                width: EditableValue::new(8.27),
                height: EditableValue::new(11.69),
                ppi: EditableValue::new(ppi),
                unit: EditableValue::new(unit),
            },
        }
    }

    pub fn size_pixels(&self) -> Vec2 {
        match self.unit {
            Unit::Pixels => self.size,
            Unit::Inches => self.size * self.ppi as f32,
            Unit::Centimeters => self.size * (self.ppi as f32 / 2.54),
        }
    }

    pub fn size(&self) -> Vec2 {
        self.size
    }

    pub fn ppi(&self) -> i32 {
        self.ppi
    }

    pub fn unit(&self) -> Unit {
        self.unit
    }

    pub fn set_size(&mut self, size: Vec2) {
        self.size = size;
    }

    pub fn set_unit(&mut self, unit: Unit) {
        let size_pixels = self.size_pixels();
        match unit {
            Unit::Pixels => self.size = size_pixels,
            Unit::Inches => self.size = size_pixels / self.ppi as f32,
            Unit::Centimeters => self.size = size_pixels / (self.ppi as f32 / 2.54),
        }
        self.unit = unit;
    }

    pub fn set_ppi(&mut self, ppi: i32) {
        self.ppi = ppi;
    }

    pub fn update_edit_state(&mut self) {
        self.edit_state.width.update_if_not_active(self.size.x);
        self.edit_state.height.update_if_not_active(self.size.y);
        self.edit_state.ppi.update_if_not_active(self.ppi);
        self.edit_state.unit.update_if_not_active(self.unit);
    }
}

impl Default for Page {
    fn default() -> Self {
        Self::a4()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasState {
    pub layers: IndexMap<LayerId, Layer>,
    pub zoom: f32,
    pub offset: Vec2,
    pub gallery_state: ImageGalleryState,
    pub multi_select: Option<MultiSelect>,
    pub page: Page,
}

impl CanvasState {
    pub fn new() -> Self {
        Self {
            layers: IndexMap::new(),
            zoom: 1.0,
            offset: Vec2::ZERO,
            gallery_state: ImageGalleryState::default(),
            multi_select: None,
            page: Page::default(),
        }
    }

    pub fn clone_with_new_widget_ids(&self) -> Self {
        let mut clone = self.clone();
        for layer in clone.layers.values_mut() {
            layer.transform_state.id = Id::random();
        }
        clone
    }

    pub fn with_photo(photo: Photo, gallery_state: ImageGalleryState) -> Self {
        let initial_rect = match photo.max_dimension() {
            crate::photo::MaxPhotoDimension::Width => {
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 1000.0 / photo.aspect_ratio()))
            }
            crate::photo::MaxPhotoDimension::Height => {
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0 * photo.aspect_ratio(), 1000.0))
            }
        };

        let canvas_photo = CanvasPhoto { photo };

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
            gallery_state,
            multi_select: None,
            page: Page::default(),
        }
    }

    fn is_layer_selected(&self, layer_id: &LayerId) -> bool {
        self.layers.get(layer_id).unwrap().selected
    }

    fn selected_layers_iter_mut(&mut self) -> impl Iterator<Item = &mut Layer> {
        self.layers.values_mut().filter(|layer| layer.selected)
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

pub struct Canvas<'a> {
    pub state: &'a mut CanvasState,
    photo_manager: Singleton<PhotoManager>,
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
            photo_manager: Dependency::get(),
            available_rect,
            history_manager,
        }
    }

    pub fn show(&mut self, ui: &mut Ui) -> Option<CanvasResponse> {
        if let Some(response) = self.handle_keys(ui.ctx()) {
            return Some(response);
        }

        let canvas_response = ui.allocate_rect(self.available_rect, Sense::click());
        let rect = canvas_response.rect;

        let is_pointer_on_canvas = self.is_pointer_on_canvas(ui);

        ui.set_clip_rect(rect);

        if let Some(_pointer_pos) = ui.ctx().pointer_hover_pos() {
            if is_pointer_on_canvas {
                ui.input(|input| {
                    let _page_rect: Rect = Rect::from_center_size(
                        rect.center() + self.state.offset * self.state.zoom,
                        self.state.page.size_pixels() * self.state.zoom,
                    );
                    if input.raw_scroll_delta.y != 0.0 {
                        // let pointer_direction = (pointer_pos - rect.center()).normalized();

                        // println!("Pointer direction: {:?}", pointer_direction);

                        // let pre_zoom_width = rect.width() * self.state.zoom;

                        // let scale_delta = if input.raw_scroll_delta.y > 0.0 {
                        //     1.1
                        // } else {
                        //     0.9
                        // };

                        // self.state.zoom *= scale_delta;

                        // let post_zoom_width = rect.width() * self.state.zoom;

                        // let multiplier_x = (pointer_pos.x - rect.center().x) / rect.center().x;
                        // let multiplier_y = (pointer_pos.y - rect.center().y) / rect.center().y;

                        // self.state.offset.x -= ((post_zoom_width - pre_zoom_width) / self.state.zoom ) * multiplier_x / self.state.zoom;
                        // self.state.offset.y -= ((post_zoom_width - pre_zoom_width) / self.state.zoom ) * multiplier_y / self.state.zoom;

                        /////////////////////////

                        // let pointer_rel_doc_center = pointer_pos - rect.center();

                        // let pointer_pos = pointer_pos;// + self.state.offset; //- rect.center().to_vec2();

                        // println!("Offset: {:?}", self.state.offset);
                        // println!("Pointer pos: {:?}", pointer_pos);

                        // let page_rect: Rect = Rect::from_center_size(
                        //     rect.center() + self.state.offset,
                        //     self.state.page.size_pixels() * self.state.zoom,
                        // );

                        // println!("Page rect: {:?}", page_rect);

                        // let mouse_to_page_left =  page_rect.left() - pointer_pos.x;
                        // let mouse_to_page_top =  page_rect.top() - pointer_pos.y;

                        // let mouse_to_page_center =  page_rect.center() - pointer_pos;

                        // println!("Mouse to page left: {}, Mouse to page top: {}", mouse_to_page_left, mouse_to_page_top);

                        // let mut scale_delta = 1.0;

                        // if input.raw_scroll_delta.y > 0.0 {
                        //     scale_delta = 1.1;
                        // } else if input.raw_scroll_delta.y < 0.0 {
                        //     scale_delta = 0.9;
                        // }

                        // self.state.zoom *= scale_delta;

                        // let page_rect: Rect = Rect::from_center_size(
                        //     rect.center() + self.state.offset,
                        //     self.state.page.size_pixels() * self.state.zoom,
                        // );

                        // let post_mouse_to_page_left = page_rect.left() - pointer_pos.x;
                        // let post_mouse_to_page_top =  page_rect.top() - pointer_pos.y;

                        // let post_mouse_to_page_center =  page_rect.center() - pointer_pos;

                        // println!("Offseting by: {}, {}", (post_mouse_to_page_left - mouse_to_page_left), (post_mouse_to_page_top - mouse_to_page_top));

                        // println!("");

                        // self.state.offset.x += (mouse_to_page_left - post_mouse_to_page_left);
                        // self.state.offset.y += ( mouse_to_page_top - post_mouse_to_page_top);

                        // self.state.offset.x += (post_mouse_to_page_center.x - mouse_to_page_center.x) * self.state.zoom;
                        // self.state.offset.y += (post_mouse_to_page_center.y - mouse_to_page_center.y) * self.state.zoom;

                        //////////////////////////////////////

                        // let mut scale_delta = 1.0;

                        // let pre_zoom_width = rect.width() * self.state.zoom;
                        // let pre_zoom_height = rect.height() * self.state.zoom;

                        // let pre_page_width = self.state.page.size_pixels().x * self.state.zoom;
                        // let pre_page_height = self.state.page.size_pixels().y * self.state.zoom;

                        // if input.raw_scroll_delta.y > 0.0 {
                        //     scale_delta = 1.1;
                        // } else if input.raw_scroll_delta.y < 0.0 {
                        //     scale_delta = 0.9;
                        // }

                        // self.state.zoom *= scale_delta;

                        // let post_zoom_width = rect.width() * self.state.zoom;
                        // let post_zoom_height = rect.height() * self.state.zoom;

                        // let post_page_width = self.state.page.size_pixels().x * self.state.zoom;
                        // let post_page_height = self.state.page.size_pixels().y * self.state.zoom;

                        // let multiplier_x = (pointer_pos.x - rect.center().x) / rect.center().x;
                        // let multiplier_y = (pointer_pos.y - rect.center().y) / rect.center().y;

                        // let offset_x = (post_page_width - pre_page_width) * (multiplier_x * 0.5);
                        // let offset_y = (post_page_height - pre_page_height) * (multiplier_y * 0.5);

                        // self.state.offset.x -= offset_x;
                        // self.state.offset.y -= offset_y;

                        ////////////////////////////////////////////

                        // let mut factor = 0.95;
                        // if input.raw_scroll_delta.y > 0.0 {
                        //     factor = 1.0/factor;
                        // }

                        // self.state.zoom += factor - 1.0;

                        // let page_rect = Rect::from_center_size(
                        //     rect.center() + self.state.offset,
                        //     self.state.page.size_pixels() * self.state.zoom,
                        // );

                        // let dx = (pointer_pos.x - page_rect.left()) * (factor - 1.0);
                        // let dy = (pointer_pos.y - page_rect.top()) * (factor - 1.0);

                        // self.state.offset.x -= dx* 2.0;
                        // self.state.offset.y -= dy * 2.0;

                        //////////////////////////

                        if input.raw_scroll_delta.y > 0.0 {
                            self.state.zoom *= 1.1;
                        } else if input.raw_scroll_delta.y < 0.0 {
                            self.state.zoom *= 0.9;
                        }
                    }
                });
            }
        }

        let page_rect: Rect = Rect::from_center_size(
            rect.center() + self.state.offset,
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

        ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
        ui.painter().rect_filled(page_rect, 0.0, Color32::WHITE);

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
                        .save_history(CanvasHistoryKind::Transform, &mut self.state);
                }
            }
        }

        self.draw_multi_select(ui, page_rect);

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

        if let Some(multi_select) = &mut self.state.multi_select {
            if multi_select.selected_layers.is_empty() {
                self.state.multi_select = None;
            } else {
                let mut transform_state = multi_select.transformable_state.clone();

                let pre_transform_rect = transform_state.rect;

                let transform_response = TransformableWidget::new(&mut transform_state).show(
                    ui,
                    rect,
                    self.state.zoom,
                    true,
                    |_ui: &mut Ui, _transformed_rect: Rect, transformable_state| {
                        // Apply transformation to the transformable_state of each layer in the multi select
                        for child in &multi_select.selected_layers {
                            let layer: &mut Layer = self.state.layers.get_mut(&child.id).unwrap();

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

                if transform_response.ended_moving
                    || transform_response.ended_resizing
                    || transform_response.ended_rotating
                {
                    self.history_manager
                        .save_history(CanvasHistoryKind::Transform, &mut self.state);
                }
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
        let layer = &mut self.state.layers.get_mut(layer_id).unwrap();
        let active = layer.selected && self.state.multi_select.is_none();

        match &mut layer.content {
            LayerContent::Photo(ref mut photo) => {
                ui.push_id(format!("CanvasPhoto_{}", layer.id), |ui| {
                    self.photo_manager.with_lock_mut(|photo_manager| {
                        if let Ok(Some(texture)) = photo_manager
                            .texture_for_photo_with_thumbail_backup(&photo.photo, ui.ctx())
                        {
                            let mut transform_state = layer.transform_state.clone();

                            let transform_response = TransformableWidget::new(&mut transform_state)
                                .show(
                                    ui,
                                    available_rect,
                                    self.state.zoom,
                                    active && !is_preview,
                                    |ui: &mut Ui, transformed_rect: Rect, _transformable_state| {
                                        let uv = Rect::from_min_max(
                                            Pos2::new(0.0, 0.0),
                                            Pos2 { x: 1.0, y: 1.0 },
                                        );

                                        let painter = ui.painter();
                                        let mut mesh = Mesh::with_texture(texture.id);

                                        // If the photo is rotated swap the width and height
                                        let mesh_rect = if photo.photo.metadata.rotation().radians()
                                            == 0.0
                                            || photo.photo.metadata.rotation().radians()
                                                == std::f32::consts::PI
                                        {
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

                                        mesh.add_rect_with_uv(mesh_rect, uv, Color32::WHITE);

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
                        } else {
                            None
                        }
                    })
                })
                .inner
            }
            LayerContent::Text(text) => {
                let mut transform_state = layer.transform_state.clone();

                let transform_response = TransformableWidget::new(&mut transform_state).show(
                    ui,
                    available_rect,
                    self.state.zoom,
                    active,
                    |ui: &mut Ui, transformed_rect: Rect, transformable_state| {
                        let painter = ui.painter();

                        let galley = painter.layout(
                            text.text.clone(),
                            FontId {
                                size: text.font_size * self.state.zoom,
                                family: text.font_id.family.clone(),
                            },
                            text.color,
                            transformed_rect.width(),
                        );

                        let text_pos = match text.alignment {
                            CanvasTextAlignment::Left => transformed_rect.left_center(),
                            CanvasTextAlignment::Center => {
                                transformed_rect.center() - galley.size() * 0.5
                            }
                            CanvasTextAlignment::Right => {
                                transformed_rect.right_center() - galley.size()
                            }
                        };

                        let text_shape = TextShape::new(text_pos, galley.clone(), Color32::BLACK)
                            .with_angle(transformable_state.rotation);

                        painter.add(text_shape);
                    },
                );

                layer.transform_state = transform_state;

                Some(transform_response)
            }
        }
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
                self.history_manager
                    .save_history(CanvasHistoryKind::DeletePhoto, &mut self.state);
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
                    .save_history(CanvasHistoryKind::Transform, &mut self.state);
            }

            // Undo/Redo
            if input.key_pressed(egui::Key::Z) && input.modifiers.ctrl {
                if input.modifiers.shift {
                    self.history_manager.redo(&mut self.state);
                } else {
                    self.history_manager.undo(&mut self.state);
                }
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

    pub fn add_photo(&mut self, photo: Photo) {
        let layer = Layer::with_photo(photo);
        self.state.layers.insert(layer.id, layer);
        self.history_manager
            .save_history(CanvasHistoryKind::AddPhoto, &mut self.state);
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
            .save_history(CanvasHistoryKind::DeselectLayer, &mut self.state);
    }

    fn deselect_all_photos(&mut self) {
        for (_, layer) in &mut self.state.layers {
            layer.selected = false;
        }
        self.history_manager
            .save_history(CanvasHistoryKind::DeselectLayer, &mut self.state);
    }
}
