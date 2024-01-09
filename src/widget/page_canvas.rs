use std::{
    cell::{Cell, RefCell},
    fmt::Display,
};

use eframe::{
    egui::{
        self, include_image, load::SizedTexture, Align, Button, CentralPanel, Context, CursorIcon,
        Image, InnerResponse, LayerId, Layout, Response, Sense, SidePanel, Ui, Widget,
    },
    emath::Rot2,
    epaint::{Color32, Mesh, Pos2, Rect, Shape, Stroke, Vec2},
};
use env_logger::fmt::Color;
use log::error;
use rayon::vec;

use crate::{
    assets::Asset,
    dependencies::{Dependency, Singleton, SingletonFor},
    photo::Photo,
    photo_manager::{self, PhotoLoadResult, PhotoManager},
    utils::{RectExt, Truncate},
    widget::placeholder::RectPlaceholder,
};

use super::{
    canvas_info::{
        layers::{Layer, Layers},
        panel::CanvasInfo,
    },
    gallery_image::GalleryImage,
    image_gallery::{self, ImageGallery, ImageGalleryState},
};

pub struct CanvasScene<'a> {
    state: &'a mut CanvasState,
}

impl<'a> CanvasScene<'a> {
    pub fn new(canvas_state: &'a mut CanvasState) -> Self {
        Self {
            state: canvas_state,
        }
    }

    pub fn show(&mut self, ctx: &Context) -> Option<CanvasResponse> {
        match SidePanel::left("image_gallery_panel")
            .default_width(300.0)
            .resizable(true)
            .show(ctx, |ui| {
                ImageGallery::show(ui, &mut self.state.gallery_state)
            })
            .inner
        {
            Some(action) => match action {
                image_gallery::ImageGalleryResponse::ViewPhotoAt(index) => {
                    // TODO
                    return Some(CanvasResponse::Exit);
                }
                image_gallery::ImageGalleryResponse::EditPhotoAt(index) => {
                    let photo_manager: Singleton<PhotoManager> = Dependency::get();

                    // TODO: Allow clicking on a pending photo
                    if let PhotoLoadResult::Ready(photo) =
                        photo_manager.with_lock(|photo_manager| photo_manager.photos[index].clone())
                    {
                        self.state.add_photo(photo.clone());
                    };
                }
            },
            None => {}
        }

        SidePanel::right("canvas_info_panel")
            .default_width(300.0)
            .resizable(true)
            .show(ctx, |ui| {
                CanvasInfo {
                    layers: &mut self.state.photos,
                }
                .ui(ui)
            });

        match CentralPanel::default()
            .show(ctx, |ui| {
                let mut canvas = Canvas::new(&mut self.state);
                canvas.show(ui)
            })
            .inner
        {
            Some(action) => match action {
                CanvasResponse::Exit => Some(CanvasResponse::Exit),
            },
            None => None,
        }
    }
}

pub enum CanvasResponse {
    Exit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasPhoto {
    pub id: usize,
    pub photo: Photo,
    pub transform_state: TransformableState,
    set_initial_position: bool,
}

impl CanvasPhoto {
    pub fn new(photo: Photo, id: usize) -> Self {
        let initial_rect = match photo.max_dimension() {
            crate::photo::MaxPhotoDimension::Width => {
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 1000.0 / photo.aspect_ratio()))
            }
            crate::photo::MaxPhotoDimension::Height => {
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0 * photo.aspect_ratio(), 1000.0))
            }
        };

        Self {
            photo,
            transform_state: TransformableState {
                rect: initial_rect,
                active_handle: None,
                is_moving: false,
                handle_mode: TransformHandleMode::default(),
                rotation: 0.0,
                last_frame_rotation: None,
                selected: false,
            },
            id,
            set_initial_position: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum CanvasHistoryKind {
    Initial,
    Transform,
    AddPhoto,
    DeletePhoto,
    Select,
}

impl Display for CanvasHistoryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CanvasHistoryKind::Initial => write!(f, "Initial"),
            CanvasHistoryKind::Transform => write!(f, "Move"),
            CanvasHistoryKind::AddPhoto => write!(f, "Add Photo"),
            CanvasHistoryKind::DeletePhoto => write!(f, "Delete Photo"),
            CanvasHistoryKind::Select => write!(f, "Select"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CanvasHistory {
    photos: Vec<Layer>,
    active_photo: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
struct HistoryManager<Kind, Value> {
    history: Vec<(Kind, Value)>,
    index: usize,
}

impl<Kind, Value> HistoryManager<Kind, Value>
where
    Kind: Display,
    Value: Clone,
{
    pub fn undo(&mut self) -> Value {
        if self.index > 0 {
            self.index = self.index - 1;
            let history = &self.history[self.index];
            history.1.clone()
        } else {
            self.history[self.index].1.clone()
        }
    }

    pub fn redo(&mut self) -> Value {
        if self.index < self.history.len() - 1 {
            self.index = self.index + 1;
            let history = &self.history[self.index];
            history.1.clone()
        } else {
            self.history[self.index].1.clone()
        }
    }

    pub fn save_history(&mut self, kind: Kind, value: Value) {
        self.history.truncate(self.index + 1);
        self.history.push((kind, value));

        self.index = self.history.len() - 1;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasState {
    photos: Vec<Layer>,
    active_photo: Option<usize>,
    zoom: f32,
    offset: Vec2,
    history_manager: HistoryManager<CanvasHistoryKind, CanvasHistory>,
    gallery_state: ImageGalleryState,
}

// History Stuff
impl CanvasState {
    pub fn undo(&mut self) {
        let new_value = self.history_manager.undo();
        self.apply_history(new_value);
    }

    pub fn redo(&mut self) {
        let new_value = self.history_manager.redo();
        self.apply_history(new_value);
    }

    pub fn save_history(&mut self, kind: CanvasHistoryKind) {
        self.history_manager.save_history(
            kind,
            CanvasHistory {
                photos: self.photos.clone(),
                active_photo: self.active_photo,
            },
        );
    }

    fn apply_history(&mut self, history: CanvasHistory) {
        self.photos = history.photos;
        self.active_photo = history.active_photo;
    }
}

impl CanvasState {
    pub fn new() -> Self {
        Self {
            photos: Vec::new(),
            active_photo: None,
            zoom: 1.0,
            offset: Vec2::ZERO,
            history_manager: HistoryManager {
                history: vec![(
                    CanvasHistoryKind::Initial,
                    CanvasHistory {
                        photos: vec![],
                        active_photo: None,
                    },
                )],
                index: 0,
            },
            gallery_state: ImageGalleryState::default(),
        }
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

        let photo = CanvasPhoto {
            photo,
            transform_state: TransformableState {
                rect: initial_rect,
                active_handle: None,
                is_moving: false,
                handle_mode: TransformHandleMode::default(),
                rotation: 0.0,
                last_frame_rotation: None,
                selected: false,
            },
            id: 0,
            set_initial_position: false,
        };

        let name = photo.photo.file_name().to_string();
        let layer = Layer {
            photo: photo,
            name: name,
            visible: true,
            locked: false,
        };

        Self {
            photos: vec![layer.clone()],
            active_photo: None,
            zoom: 1.0,
            offset: Vec2::ZERO,
            history_manager: HistoryManager {
                history: vec![(
                    CanvasHistoryKind::Initial,
                    CanvasHistory {
                        photos: vec![layer],
                        active_photo: None,
                    },
                )],
                index: 0,
            },
            gallery_state: gallery_state,
        }
    }

    pub fn add_photo(&mut self, photo: Photo) {
        self.photos
            .push(Layer::with_photo(photo, self.photos.len()));
        self.save_history(CanvasHistoryKind::AddPhoto);
    }
}

pub struct Canvas<'a> {
    pub state: &'a mut CanvasState,
    photo_manager: Singleton<PhotoManager>,
}

impl<'a> Canvas<'a> {
    pub fn new(state: &'a mut CanvasState) -> Self {
        Self {
            state,
            photo_manager: Dependency::get(),
        }
    }

    pub fn show(&mut self, ui: &mut Ui) -> Option<CanvasResponse> {
        if let Some(response) = self.handle_keys(ui.ctx()) {
            return Some(response);
        }

        let available_rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(available_rect, Sense::click());
        let rect = response.rect;

        ui.set_clip_rect(rect);

        if let Some(pointer_pos) = ui.ctx().pointer_hover_pos() {
            if rect.contains(pointer_pos) {
                ui.input(|input| {
                    self.state.zoom += input.scroll_delta.y * 0.005;
                    self.state.zoom = self.state.zoom.max(0.1);
                });
            }
        }

        ui.input(|input| {
            if input.key_down(egui::Key::Space) {
                self.state.offset += input.pointer.delta();
                true
            } else {
                false
            }
        });

        ui.painter().rect_filled(rect, 0.0, Color32::BLACK);

        {
            // TEMP: page placeholder

            let page_rect = Rect::from_center_size(rect.center(), Vec2::new(1000.0, 1500.0));
            let page_rect = page_rect.translate(self.state.offset);

            let expansion = ((page_rect.size() * self.state.zoom) - page_rect.size()) * 0.5;
            let page_rect = page_rect.expand2(expansion);

            ui.painter().rect_filled(page_rect, 0.0, Color32::WHITE);
        }

        // Reset the cursor icon so it can be set by the transform widgets
        ui.ctx().set_cursor_icon(CursorIcon::Default);

        let mut transform_responses = Vec::new();

        for layer in &mut self.state.photos.iter_mut() {
            // Move the photo to the center of the canvas if it hasn't been moved yet
            if !layer.photo.set_initial_position {
                layer.photo.transform_state.rect.set_center(rect.center());
                layer.photo.set_initial_position = true;
            }

            ui.push_id(format!("CanvasPhoto_{}", layer.photo.id), |ui| {
                self.photo_manager.with_lock_mut(|photo_manager| {
                    if let Ok(Some(texture)) =
                        photo_manager.texture_for(&layer.photo.photo, &ui.ctx())
                    {
                        let mut transform_state = layer.photo.transform_state.clone();

                        if layer.photo.id != self.state.active_photo.unwrap_or(usize::MAX) {
                            transform_state.selected = false;
                        }

                        let transform_response = TransformableWidget::new(&mut transform_state)
                            .show(
                                ui,
                                available_rect,
                                self.state.zoom,
                                self.state.offset,
                                |ui: &mut Ui, transformed_rect: Rect, transformable_state| {
                                    let uv = Rect::from_min_max(
                                        Pos2::new(0.0, 0.0),
                                        Pos2 { x: 1.0, y: 1.0 },
                                    );

                                    let painter = ui.painter();
                                    let mut mesh = Mesh::with_texture(texture.id);

                                    // If the photo is rotated swap the width and height
                                    let mesh_rect =
                                        if layer.photo.photo.metadata.rotation().radians() == 0.0
                                            || layer.photo.photo.metadata.rotation().radians()
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

                                    let mesh_center =
                                        mesh_rect.min + Vec2::splat(0.5) * mesh_rect.size();

                                    mesh.rotate(
                                        Rot2::from_angle(
                                            layer.photo.photo.metadata.rotation().radians(),
                                        ),
                                        mesh_center,
                                    );
                                    mesh.rotate(
                                        Rot2::from_angle(layer.photo.transform_state.rotation),
                                        mesh_center,
                                    );

                                    painter.add(Shape::mesh(mesh));

                                    // If the canvas was clicked but not on the photo then deselect the photo
                                    if response.clicked()
                                        && !transformed_rect.contains(
                                            response.interact_pointer_pos().unwrap_or(Pos2::ZERO),
                                        )
                                        && self.state.active_photo == Some(layer.photo.id)
                                    {
                                        self.state.active_photo = None;
                                    } else if transformable_state.selected
                                        && self.state.active_photo != Some(layer.photo.id)
                                    {
                                        // If the photo was selected this frame then set it as the active photo
                                        // and deselect all other photos
                                        self.state.active_photo = Some(layer.photo.id);
                                    }
                                },
                            );

                        layer.photo.transform_state = transform_state;

                        transform_responses.push(transform_response);
                    }
                });
            });
        }

        for transform_response in transform_responses {
            if transform_response.ended_moving
                || transform_response.ended_resizing
                || transform_response.ended_rotating
            {
                self.state.save_history(CanvasHistoryKind::Transform);
                break;
            }
        }

        None
    }

    fn handle_keys(&mut self, ctx: &Context) -> Option<CanvasResponse> {
        ctx.input(|input| {
            // Exit the canvas
            if input.key_pressed(egui::Key::Backspace) && input.modifiers.ctrl {
                return Some(CanvasResponse::Exit);
            }

            // Clear the selected photo
            if input.key_pressed(egui::Key::Escape) {
                self.state.active_photo = None;
            }

            // Delete the selected photo
            if input.key_pressed(egui::Key::Delete) {
                if let Some(active_photo) = self.state.active_photo {
                    self.state.photos.remove(active_photo);
                    self.state.active_photo = None;

                    // Update the ids of the photos since they are just indices
                    for (i, photo) in self.state.photos.iter_mut().enumerate() {
                        photo.photo.id = i;
                    }

                    self.state.save_history(CanvasHistoryKind::DeletePhoto);
                }
            }

            // Move the selected photo
            if let Some(active_photo) = self.state.active_photo {
                // Handle movement via arrow keys
                {
                    let distance = if input.modifiers.shift { 10.0 } else { 1.0 };

                    let mut transform_state = self.state.photos[active_photo]
                        .photo
                        .transform_state
                        .clone();

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

                    self.state.photos[active_photo].photo.transform_state = transform_state;

                    // Once the arrow key is released then log the history
                    if input.key_released(egui::Key::ArrowLeft)
                        || input.key_released(egui::Key::ArrowRight)
                        || input.key_released(egui::Key::ArrowUp)
                        || input.key_released(egui::Key::ArrowDown)
                    {
                        self.state.save_history(CanvasHistoryKind::Transform);
                    }
                }

                // Switch to scale mode
                if input.key_pressed(egui::Key::S) {
                    // TODO should the resize mode be persisted? Probably.
                    self.state.photos[active_photo]
                        .photo
                        .transform_state
                        .handle_mode = TransformHandleMode::Resize(ResizeMode::MirrorAxis);
                }

                // Switch to rotate mode
                if input.key_pressed(egui::Key::R) {
                    self.state.photos[active_photo]
                        .photo
                        .transform_state
                        .handle_mode = TransformHandleMode::Rotate;
                }
            }

            // Undo/Redo
            if input.key_pressed(egui::Key::Z) && input.modifiers.ctrl {
                if input.modifiers.shift {
                    self.state.redo();
                } else {
                    self.state.undo();
                }
            }

            None
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum TransformHandle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    MiddleTop,
    MiddleBottom,
    MiddleLeft,
    MiddleRight,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum ResizeMode {
    Free,
    MirrorAxis,
    ConstrainedAspectRatio,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum TransformHandleMode {
    Resize(ResizeMode),
    Rotate,
}

impl Default for TransformHandleMode {
    fn default() -> Self {
        Self::Resize(ResizeMode::Free)
    }
}

impl TransformHandle {
    fn cursor(&self) -> CursorIcon {
        match self {
            TransformHandle::TopLeft => CursorIcon::ResizeNorthWest,
            TransformHandle::TopRight => CursorIcon::ResizeNorthEast,
            TransformHandle::BottomLeft => CursorIcon::ResizeSouthWest,
            TransformHandle::BottomRight => CursorIcon::ResizeSouthEast,
            TransformHandle::MiddleTop => CursorIcon::ResizeRow,
            TransformHandle::MiddleBottom => CursorIcon::ResizeRow,
            TransformHandle::MiddleLeft => CursorIcon::ResizeColumn,
            TransformHandle::MiddleRight => CursorIcon::ResizeColumn,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransformableState {
    pub rect: Rect,
    pub active_handle: Option<TransformHandle>,
    pub is_moving: bool,
    pub handle_mode: TransformHandleMode,
    pub rotation: f32,
    pub last_frame_rotation: Option<f32>,
    pub selected: bool,
}

pub struct TransformableWidget<'a> {
    pub state: &'a mut TransformableState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransformableWidgetResponseAction {
    PushHistory,
}

#[derive(Debug, Clone)]
pub struct TransformableWidgetResponse {
    inner: Response,
    began_moving: bool,
    began_resizing: bool,
    began_rotating: bool,
    ended_moving: bool,
    ended_resizing: bool,
    ended_rotating: bool,
}

impl<'a> TransformableWidget<'a> {
    const HANDLE_SIZE: Vec2 = Vec2::splat(10.0);

    pub fn new(state: &'a mut TransformableState) -> Self {
        Self { state }
    }

    pub fn show<R>(
        &mut self,
        ui: &mut Ui,
        container_rect: Rect,
        global_scale: f32,
        global_offset: Vec2,
        add_contents: impl FnOnce(&mut Ui, Rect, &TransformableState) -> R,
    ) -> TransformableWidgetResponse {
        let initial_is_moving = self.state.is_moving;
        let initial_active_handle = self.state.active_handle;
        let initial_mode = self.state.handle_mode;

        let photo_center_relative_to_page = container_rect.center() - self.state.rect.center();

        // Scale the position of the center of the photo
        let scaled_photo_center = photo_center_relative_to_page * global_scale;

        // Translate photo to the new center position, adjusted for the global offset
        let translated_rect_center = container_rect.center() + global_offset - scaled_photo_center;

        // Scale the size of the photo
        let scaled_photo_size = self.state.rect.size() * global_scale;

        // Create the new scaled and translated rect for the photo
        let pre_rotated_inner_content_rect = Rect::from_min_size(
            translated_rect_center - scaled_photo_size / 2.0,
            scaled_photo_size,
        );

        let rotated_inner_content_rect =
            pre_rotated_inner_content_rect.rotate_bb_around_center(self.state.rotation);

        let response = if self.state.selected {
            // Draw the mode selector above the inner content
            let mode_selector_response =
                self.draw_handle_mode_selector(ui, rotated_inner_content_rect.center_top());

            mode_selector_response
                .union(ui.allocate_rect(rotated_inner_content_rect, Sense::click_and_drag()))
        } else {
            ui.allocate_rect(rotated_inner_content_rect, Sense::click_and_drag())
        };

        let rect = response.rect;

        let middle_point = |p1: Pos2, p2: Pos2| p1 + (p2 - p1) / 2.0;

        let handles = [
            (
                TransformHandle::TopLeft,
                rotated_inner_content_rect.left_top() - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::TopRight,
                rotated_inner_content_rect.right_top() - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::BottomLeft,
                rotated_inner_content_rect.left_bottom() - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::BottomRight,
                rotated_inner_content_rect.right_bottom() - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::MiddleTop,
                middle_point(
                    rotated_inner_content_rect.left_top(),
                    rotated_inner_content_rect.right_top(),
                ) - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::MiddleBottom,
                middle_point(
                    rotated_inner_content_rect.left_bottom(),
                    rotated_inner_content_rect.right_bottom(),
                ) - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::MiddleLeft,
                middle_point(
                    rotated_inner_content_rect.left_top(),
                    rotated_inner_content_rect.left_bottom(),
                ) - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::MiddleRight,
                middle_point(
                    rotated_inner_content_rect.right_top(),
                    rotated_inner_content_rect.right_bottom(),
                ) - Self::HANDLE_SIZE / 2.0,
            ),
        ];

        // Interact with an expanded rect to include the handles which are partially outside the rect
        let interact_response: Response = ui.interact(
            rotated_inner_content_rect.expand(Self::HANDLE_SIZE.x / 2.0),
            response.id,
            Sense::click_and_drag(),
        );

        if interact_response.is_pointer_button_down_on() {
            self.state.selected = true;
        }

        if self.state.selected {
            for (handle, rotated_handle_pos) in &handles {
                let handle_rect: Rect = Rect::from_min_size(*rotated_handle_pos, Self::HANDLE_SIZE);

                if !interact_response.is_pointer_button_down_on()
                    && self.state.active_handle == Some(*handle)
                {
                    self.state.active_handle = None;
                    self.state.last_frame_rotation = None;
                }

                if interact_response
                    .interact_pointer_pos()
                    .and_then(|pos| Some(handle_rect.contains(pos)))
                    .unwrap_or(false)
                    || self.state.active_handle == Some(*handle)
                {
                    let delta = interact_response.drag_delta() / global_scale;

                    let (shift_pressed, alt_pressed) = ui
                        .ctx()
                        .input(|input| (input.modifiers.shift, input.modifiers.alt));

                    match (self.state.handle_mode, shift_pressed, alt_pressed) {
                        (TransformHandleMode::Resize(ResizeMode::MirrorAxis), _, _)
                        | (TransformHandleMode::Resize(ResizeMode::Free), false, true) => {
                            let mut new_rect = self.state.rect;

                            match handle {
                                TransformHandle::TopLeft => {
                                    new_rect.min.x += delta.x;
                                    new_rect.min.y += delta.y;

                                    new_rect.max.x -= delta.x;
                                    new_rect.max.y -= delta.y;
                                }
                                TransformHandle::TopRight => {
                                    new_rect.max.x += delta.x;
                                    new_rect.min.y += delta.y;

                                    new_rect.min.x -= delta.x;
                                    new_rect.max.y -= delta.y;
                                }
                                TransformHandle::BottomLeft => {
                                    new_rect.min.x += delta.x;
                                    new_rect.max.y += delta.y;

                                    new_rect.max.x -= delta.x;
                                    new_rect.min.y -= delta.y;
                                }
                                TransformHandle::BottomRight => {
                                    new_rect.max.x += delta.x;
                                    new_rect.max.y += delta.y;

                                    new_rect.min.x -= delta.x;
                                    new_rect.min.y -= delta.y;
                                }
                                TransformHandle::MiddleTop => {
                                    new_rect.min.y += delta.y;
                                    new_rect.max.y -= delta.y;
                                }
                                TransformHandle::MiddleBottom => {
                                    new_rect.max.y += delta.y;
                                    new_rect.min.y -= delta.y;
                                }
                                TransformHandle::MiddleLeft => {
                                    new_rect.min.x += delta.x;
                                    new_rect.max.x -= delta.x;
                                }
                                TransformHandle::MiddleRight => {
                                    new_rect.max.x += delta.x;
                                    new_rect.min.x -= delta.x;
                                }
                            };

                            self.state.active_handle = Some(*handle);
                            self.state.rect = new_rect;
                        }
                        (TransformHandleMode::Resize(ResizeMode::ConstrainedAspectRatio), _, _)
                        | (TransformHandleMode::Resize(ResizeMode::Free), true, false) => {
                            let mut new_rect = self.state.rect;

                            let (ratio_x, ratio_y) = if new_rect.width() > new_rect.height() {
                                (new_rect.width() / new_rect.height(), 1.0)
                            } else {
                                (1.0, new_rect.height() / new_rect.width())
                            };

                            let max_delta = delta.x.min(delta.y);
                            let delta_x = max_delta * ratio_x;
                            let delta_y = max_delta * ratio_y;

                            match handle {
                                TransformHandle::TopLeft => {
                                    new_rect.min.x += delta_x;
                                    new_rect.min.y += delta_y;
                                }
                                TransformHandle::TopRight => {
                                    let max_delta = delta.x.abs().max(delta.y.abs());

                                    if delta.x.abs() > delta.y.abs() && delta.x > 0.0 {
                                        new_rect.max.x += max_delta * ratio_x;
                                        new_rect.min.y -= max_delta * ratio_y;
                                    } else if delta.x.abs() > delta.y.abs() && delta.x < 0.0 {
                                        new_rect.max.x -= max_delta * ratio_x;
                                        new_rect.min.y += max_delta * ratio_y;
                                    } else if delta.y.abs() > delta.x.abs() && delta.y > 0.0 {
                                        new_rect.max.x -= max_delta * ratio_x;
                                        new_rect.min.y += max_delta * ratio_y;
                                    } else if delta.y.abs() > delta.x.abs() && delta.y < 0.0 {
                                        new_rect.max.x += max_delta * ratio_x;
                                        new_rect.min.y -= max_delta * ratio_y;
                                    }
                                }
                                TransformHandle::BottomLeft => {
                                    let max_delta = delta.x.abs().max(delta.y.abs());

                                    if delta.x.abs() > delta.y.abs() && delta.x > 0.0 {
                                        new_rect.min.x += max_delta * ratio_x;
                                        new_rect.max.y -= max_delta * ratio_y;
                                    } else if delta.x.abs() > delta.y.abs() && delta.x < 0.0 {
                                        new_rect.min.x -= max_delta * ratio_x;
                                        new_rect.max.y += max_delta * ratio_y;
                                    } else if delta.y.abs() > delta.x.abs() && delta.y > 0.0 {
                                        new_rect.min.x -= max_delta * ratio_x;
                                        new_rect.max.y += max_delta * ratio_y;
                                    } else if delta.y.abs() > delta.x.abs() && delta.y < 0.0 {
                                        new_rect.min.x += max_delta * ratio_x;
                                        new_rect.max.y -= max_delta * ratio_y;
                                    }
                                }
                                TransformHandle::BottomRight => {
                                    new_rect.max.x += delta_x;
                                    new_rect.max.y += delta_y;
                                }
                                TransformHandle::MiddleTop | TransformHandle::MiddleBottom => {
                                    new_rect.min.y -= delta.y * ratio_y * -1.0;
                                    new_rect.max.y += delta.y * ratio_y * -1.0;
                                    new_rect.min.x -= delta.y * ratio_x * -1.0;
                                    new_rect.max.x += delta.y * ratio_x * -1.0;
                                }
                                TransformHandle::MiddleLeft | TransformHandle::MiddleRight => {
                                    new_rect.min.y += delta.x * ratio_y;
                                    new_rect.max.y -= delta.x * ratio_y;
                                    new_rect.min.x += delta.x * ratio_x;
                                    new_rect.max.x -= delta.x * ratio_x;
                                }
                            };

                            self.state.active_handle = Some(*handle);
                            self.state.rect = new_rect;
                        }
                        (TransformHandleMode::Resize(ResizeMode::Free), _, _) => {
                            let mut new_rect = self.state.rect;

                            match handle {
                                TransformHandle::TopLeft => {
                                    new_rect.min.x += delta.x;
                                    new_rect.min.y += delta.y;
                                }
                                TransformHandle::TopRight => {
                                    new_rect.max.x += delta.x;
                                    new_rect.min.y += delta.y;
                                }
                                TransformHandle::BottomLeft => {
                                    new_rect.min.x += delta.x;
                                    new_rect.max.y += delta.y;
                                }
                                TransformHandle::BottomRight => {
                                    new_rect.max.x += delta.x;
                                    new_rect.max.y += delta.y;
                                }
                                TransformHandle::MiddleTop => {
                                    new_rect.min.y += delta.y;
                                }
                                TransformHandle::MiddleBottom => {
                                    new_rect.max.y += delta.y;
                                }
                                TransformHandle::MiddleLeft => {
                                    new_rect.min.x += delta.x;
                                }
                                TransformHandle::MiddleRight => {
                                    new_rect.max.x += delta.x;
                                }
                            };

                            self.state.active_handle = Some(*handle);
                            self.state.rect = new_rect;
                        }
                        (TransformHandleMode::Rotate, _, _) => {
                            if let Some(cursor_pos) = interact_response.interact_pointer_pos() {
                                let from_cursor_to_center =
                                    cursor_pos - rotated_inner_content_rect.center();

                                let from_rotated_handle_to_center =
                                    Rect::from_min_size(*rotated_handle_pos, Self::HANDLE_SIZE)
                                        .center()
                                        - rotated_inner_content_rect.center();

                                let rotated_signed_angle =
                                    f32::atan2(from_cursor_to_center.y, from_cursor_to_center.x)
                                        - f32::atan2(
                                            from_rotated_handle_to_center.y,
                                            from_rotated_handle_to_center.x,
                                        );

                                self.state.rotation += rotated_signed_angle
                                    - self.state.last_frame_rotation.unwrap_or(0.0);
                                self.state.active_handle = Some(*handle);
                                self.state.last_frame_rotation = Some(rotated_signed_angle);
                            }
                        }
                    }
                }
            }

            if self.state.active_handle.is_none() {
                if interact_response.is_pointer_button_down_on()
                    && (self.state.is_moving
                        || interact_response
                            .interact_pointer_pos()
                            .and_then(|pos| Some(rect.contains(pos)))
                            .unwrap_or(false))
                {
                    let delta = interact_response.drag_delta() / global_scale;
                    self.state.rect = self.state.rect.translate(delta);
                    self.state.is_moving = true;
                } else {
                    self.state.is_moving = false;
                }

                if interact_response.double_clicked() {
                    match self.state.handle_mode {
                        TransformHandleMode::Resize(_) => {
                            self.state.handle_mode = TransformHandleMode::Rotate
                        }
                        TransformHandleMode::Rotate => {
                            self.state.handle_mode = TransformHandleMode::Resize(ResizeMode::Free)
                        }
                    }
                }
            } else {
                self.state.is_moving = false;
            }
        } else {
            self.state.is_moving = false;
            self.state.active_handle = None;
        }

        let _inner_response = add_contents(ui, pre_rotated_inner_content_rect, self.state);

        if self.state.selected {
            self.draw_bounds_with_handles(ui, &rotated_inner_content_rect, &handles);
            self.update_cursor(ui, &rotated_inner_content_rect, &handles);
        }

        TransformableWidgetResponse {
            inner: response,
            began_moving: !initial_is_moving && self.state.is_moving,
            began_resizing: initial_active_handle.is_none()
                && self.state.active_handle.is_some()
                && matches!(initial_mode, TransformHandleMode::Resize(_)),
            began_rotating: initial_active_handle.is_none()
                && self.state.active_handle.is_some()
                && matches!(initial_mode, TransformHandleMode::Rotate),
            ended_moving: initial_is_moving && !self.state.is_moving,
            ended_resizing: initial_active_handle.is_some()
                && self.state.active_handle.is_none()
                && matches!(initial_mode, TransformHandleMode::Resize(_)),
            ended_rotating: initial_active_handle.is_some()
                && self.state.active_handle.is_none()
                && matches!(initial_mode, TransformHandleMode::Rotate),
        }
    }

    fn update_cursor(
        &self,
        ui: &mut Ui,
        rotated_inner_content_rect: &Rect,
        handles: &[(TransformHandle, Pos2)],
    ) {
        ui.ctx().pointer_latest_pos().map(|pos| {
            for (handle, handle_pos) in handles {
                let handle_rect = Rect::from_min_size(*handle_pos, Self::HANDLE_SIZE);
                if handle_rect.contains(pos) {
                    match self.state.handle_mode {
                        TransformHandleMode::Resize(_) => {
                            ui.ctx().set_cursor_icon(handle.cursor());
                        }
                        TransformHandleMode::Rotate => {
                            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
                        }
                    }
                    break;
                } else if rotated_inner_content_rect.contains(pos) {
                    ui.ctx().set_cursor_icon(CursorIcon::Move);
                }
            }
        });
    }

    fn draw_bounds_with_handles(
        &self,
        ui: &mut Ui,
        rotated_content_rect: &Rect,
        handles: &[(TransformHandle, Pos2)],
    ) {
        ui.painter()
            .rect_stroke(*rotated_content_rect, 0.0, Stroke::new(2.0, Color32::GRAY));

        // Draw the resize handles
        for (_, handle_pos) in handles {
            let handle_rect = Rect::from_min_size(*handle_pos, Self::HANDLE_SIZE);
            ui.painter().rect(
                handle_rect,
                1.0,
                Color32::WHITE,
                Stroke::new(2.0, Color32::BLACK),
            );
        }
    }

    fn draw_handle_mode_selector(&mut self, ui: &mut Ui, bottom_center_origin: Pos2) -> Response {
        let width = 100.0;
        let height = 60.0;
        let margin_bottom = 20.0;
        let button_padding = 15.0;

        let button_size = Vec2::new(height - button_padding * 2.0, height - button_padding * 2.0);

        let response = ui.allocate_rect(
            Rect::from_points(&[
                bottom_center_origin + Vec2::new(0.0, -margin_bottom),
                bottom_center_origin + Vec2::new(-width * 0.5, -height),
                bottom_center_origin + Vec2::new(width * 0.5, -height),
            ]),
            Sense::hover(),
        );

        ui.painter()
            .rect(response.rect, 4.0, Color32::from_gray(40), Stroke::NONE);

        let left_half_rect =
            Rect::from_points(&[response.rect.left_top(), response.rect.center_bottom()]);

        let right_half_rect =
            Rect::from_points(&[response.rect.center_bottom(), response.rect.right_top()]);

        if ui
            .put(
                Rect::from_center_size(left_half_rect.center(), button_size),
                Button::image(
                    Image::from(Asset::resize())
                        .tint(Color32::WHITE)
                        .fit_to_exact_size(button_size * 0.8),
                )
                .fill(
                    if matches!(self.state.handle_mode, TransformHandleMode::Resize(_)) {
                        Color32::from_gray(100)
                    } else {
                        Color32::from_gray(50)
                    },
                )
                .sense(Sense::click()),
            )
            .clicked()
        {
            self.state.handle_mode = TransformHandleMode::Resize(ResizeMode::Free);
        }

        if ui
            .put(
                Rect::from_center_size(right_half_rect.center(), button_size),
                Button::image(
                    Image::from(Asset::rotate())
                        .tint(Color32::WHITE)
                        .fit_to_exact_size(button_size * 0.8),
                )
                .fill(
                    if matches!(self.state.handle_mode, TransformHandleMode::Rotate) {
                        Color32::from_gray(100)
                    } else {
                        Color32::from_gray(50)
                    },
                )
                .sense(Sense::click()),
            )
            .clicked()
        {
            self.state.handle_mode = TransformHandleMode::Rotate;
        }

        response
    }
}
