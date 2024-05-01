use std::hash::Hasher;

use eframe::epaint::Color32;
use egui::{CursorIcon, FontId, Id, Image, Pos2, Rect, Vec2};
use indexmap::IndexMap;
use strum_macros::{Display, EnumIter};

use crate::{
    cursor_manager::CursorManager,
    dependencies::{Dependency, Singleton, SingletonFor},
    history::HistoricallyEqual,
    id::{next_layer_id, LayerId},
    model::editable_value::EditableValue,
    photo::Photo,
    photo_manager::PhotoManager,
    template::TemplateRegion,
    utils::{IdExt, Toggle},
    widget::{
        page_canvas::CanvasPhoto,
        placeholder::RectPlaceholder,
        transformable::{TransformHandleMode, TransformableState},
    },
};
use egui_dnd::{dnd, utils::shift_vec};

use core::hash::Hash;

#[derive(Debug, Clone, PartialEq)]
pub struct LayerTransformEditState {
    pub x: EditableValue<f32>,
    pub y: EditableValue<f32>,
    pub width: EditableValue<f32>,
    pub height: EditableValue<f32>,
    pub rotation: EditableValue<f32>,
}

impl From<&TransformableState> for LayerTransformEditState {
    fn from(state: &TransformableState) -> Self {
        Self {
            x: EditableValue::new(state.rect.left_top().x),
            y: EditableValue::new(state.rect.left_top().y),
            width: EditableValue::new(state.rect.width()),
            height: EditableValue::new(state.rect.height()),
            rotation: EditableValue::new(state.rotation.to_degrees()),
        }
    }
}

impl LayerTransformEditState {
    pub fn update(&mut self, state: &TransformableState) {
        self.x.update_if_not_active(state.rect.left_top().x);
        self.y.update_if_not_active(state.rect.left_top().y);
        self.width.update_if_not_active(state.rect.width());
        self.height.update_if_not_active(state.rect.height());
        self.rotation
            .update_if_not_active(state.rotation.to_degrees());
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasTextEditState {
    pub font_size: EditableValue<f32>,
}

impl CanvasTextEditState {
    pub fn new(font_size: f32) -> Self {
        Self {
            font_size: EditableValue::new(font_size),
        }
    }

    pub fn update(&mut self, font_size: f32) {
        self.font_size.update_if_not_active(font_size);
    }
}

#[derive(Debug, Clone, PartialEq, Display, EnumIter)]
pub enum CanvasTextAlignment {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasText {
    pub text: String,
    pub font_size: f32,
    pub font_id: FontId,
    pub alignment: CanvasTextAlignment,
    pub color: Color32,
    pub edit_state: CanvasTextEditState,
}

impl CanvasText {
    pub fn new(
        text: String,
        font_size: f32,
        font_family: FontId,
        alignment: CanvasTextAlignment,
        color: Color32,
    ) -> Self {
        Self {
            text,
            font_size,
            font_id: font_family,
            edit_state: CanvasTextEditState::new(font_size),
            alignment,
            color,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LayerContent {
    Photo(CanvasPhoto),
    Text(CanvasText),
    TemplatePhoto {
        region: TemplateRegion,
        photo: Option<CanvasPhoto>,
    },
    TemplateText {
        region: TemplateRegion,
        text: CanvasText,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub content: LayerContent,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub selected: bool,
    pub id: LayerId,
    pub transform_edit_state: LayerTransformEditState,
    pub transform_state: TransformableState,
}

impl Layer {
    pub fn with_photo(photo: Photo) -> Self {
        let name = photo.file_name().to_string();

        let rect = match photo.max_dimension() {
            crate::photo::MaxPhotoDimension::Width => {
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 1000.0 / photo.aspect_ratio()))
            }
            crate::photo::MaxPhotoDimension::Height => {
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0 * photo.aspect_ratio(), 1000.0))
            }
        };

        let canvas_photo = CanvasPhoto::new(photo);

        let transform_state = TransformableState {
            rect,
            active_handle: None,
            is_moving: false,
            handle_mode: TransformHandleMode::default(),
            rotation: 0.0,
            last_frame_rotation: 0.0,
            change_in_rotation: None,
            id: Id::random(),
        };
        let transform_edit_state = LayerTransformEditState::from(&transform_state);
        Self {
            content: LayerContent::Photo(canvas_photo),
            name,
            visible: true,
            locked: false,
            selected: false,
            id: next_layer_id(),
            transform_edit_state,
            transform_state,
        }
    }

    pub fn new_text_layer() -> Self {
        let text = CanvasText::new(
            "New Text Layer".to_string(),
            20.0,
            FontId::default(),
            CanvasTextAlignment::Left,
            Color32::BLACK,
        );
        let transform_state = TransformableState {
            rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 100.0)),
            active_handle: None,
            is_moving: false,
            handle_mode: TransformHandleMode::default(),
            rotation: 0.0,
            last_frame_rotation: 0.0,
            change_in_rotation: None,
            id: Id::random(),
        };
        let transform_edit_state = LayerTransformEditState::from(&transform_state);
        Self {
            content: LayerContent::Text(text),
            name: "New Text Layer".to_string(),
            visible: true,
            locked: false,
            selected: false,
            id: next_layer_id(),
            transform_edit_state,
            transform_state,
        }
    }
}

impl HistoricallyEqual for Layer {
    fn historically_eqaul_to(&self, other: &Self) -> bool {
        let layer_content_equal = match (&self.content, &other.content) {
            (LayerContent::Photo(photo), LayerContent::Photo(other_photo)) => {
                photo.photo == other_photo.photo
            }
            (LayerContent::Text(text), LayerContent::Text(other_text)) => {
                text.text == other_text.text
                    && text.font_size == other_text.font_size
                    && text.font_id == other_text.font_id
                    && text.alignment == other_text.alignment
                    && text.color == other_text.color
            }
            _ => false,
        };

        layer_content_equal
            && self.name == other.name
            && self.visible == other.visible
            && self.locked == other.locked
            && self.selected == other.selected
            && self.id == other.id
            && self.transform_state == other.transform_state
    }
}

pub enum LayersResponse {
    SelectedLayer(LayerId),
    None,
}

#[derive(Debug)]
pub struct Layers<'a> {
    layers: &'a mut IndexMap<LayerId, Layer>,
    photo_manager: Singleton<PhotoManager>,
}

impl Hash for Layer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<'a> Layers<'a> {
    pub fn new(layers: &'a mut IndexMap<LayerId, Layer>) -> Self {
        Self {
            layers,
            photo_manager: Dependency::get(),
        }
    }

    pub fn show(&mut self, ui: &mut eframe::egui::Ui) -> LayersResponse {
        let mut selected_layer_id = None;

        ui.vertical(|ui| {
            let dnd_response = dnd(ui, "layers_dnd").show(
                self.layers.iter().rev(),
                |ui, (_layer_id, layer), handle, _state| {
                    // for layer in self.layers.iter_mut() {
                    let _layer_response = ui.horizontal(|ui| {
                        handle.ui(ui, |ui| {
                            ui.set_height(60.0);

                            if layer.selected {
                                let painter = ui.painter();
                                painter.rect_filled(
                                    ui.max_rect(),
                                    0.0,
                                    Color32::from_rgb(0, 0, 255),
                                );
                            }

                            match &layer.content {
                                LayerContent::Photo(canvas_photo) => {
                                    let texture_id =
                                        self.photo_manager.with_lock_mut(|photo_manager| {
                                            photo_manager.thumbnail_texture_for(
                                                &canvas_photo.photo,
                                                ui.ctx(),
                                            )
                                        });

                                    let image_size =
                                        Vec2::from(canvas_photo.photo.size_with_max_size(50.0));

                                    match texture_id {
                                        Ok(Some(texture_id)) => {
                                            let image = Image::from_texture(texture_id)
                                                .rotate(
                                                    canvas_photo
                                                        .photo
                                                        .metadata
                                                        .rotation()
                                                        .radians(),
                                                    Vec2::splat(0.5),
                                                )
                                                .fit_to_exact_size(image_size);
                                            ui.add_sized(Vec2::new(70.0, 50.0), image);
                                        }
                                        _ => {
                                            ui.add_sized(
                                                Vec2::new(70.0, 50.0),
                                                RectPlaceholder::new(image_size, Color32::GRAY),
                                            );
                                        }
                                    };
                                }
                                LayerContent::Text(_) => {
                                    ui.label("Text");
                                }
                                LayerContent::TemplatePhoto { region, photo } => {
                                    ui.label("Template Photo");
                                }
                                LayerContent::TemplateText { region, text } => {
                                    ui.label("Template Text");
                                },
                            }

                            ui.label(&layer.name);

                            ui.add_space(10.0);

                            if ui.rect_contains_pointer(ui.max_rect()) {
                                Dependency::<CursorManager>::get().with_lock_mut(
                                    |cursor_manager| {
                                        cursor_manager.set_cursor(CursorIcon::PointingHand);
                                    },
                                );
                            }

                            if ui.input(|i| i.pointer.primary_clicked())
                                && ui.rect_contains_pointer(ui.max_rect())
                            {
                                selected_layer_id = Some(layer.id);
                            }
                        });
                    });
                    ui.separator();
                },
            );

            if let Some(drag_update) = dnd_response.final_update() {
                let mut shifted_values = self.layers.values().collect::<Vec<_>>();
                shift_vec(drag_update.from, drag_update.to, &mut shifted_values);
                *self.layers = shifted_values
                    .into_iter()
                    .map(|layer| (layer.id, layer.clone()))
                    .collect();
            }
        });

        if let Some(selected_layer_id) = selected_layer_id {
            if ui.ctx().input(|input| input.modifiers.ctrl) {
                self.layers
                    .get_mut(&selected_layer_id)
                    .unwrap()
                    .selected
                    .toggle();
            } else {
                for (_, layer) in self.layers.iter_mut() {
                    layer.selected = layer.id == selected_layer_id;
                }
            }
        }

        match selected_layer_id {
            Some(selected_layer_id) => LayersResponse::SelectedLayer(selected_layer_id),
            None => LayersResponse::None,
        }
    }
}
