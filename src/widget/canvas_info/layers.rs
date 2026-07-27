use std::hash::Hasher;

use eframe::epaint::{Color32, Stroke};
use egui::{
    Align2, CornerRadius, FontId, Id, Pos2, Rect, Response, Sense, StrokeKind, TextFormat, Vec2,
    text::{LayoutJob, TextWrapping},
};
use indexmap::IndexMap;
use strum_macros::{Display, EnumIter};

use crate::{
    assets::Asset,
    id::{LayerId, next_layer_id},
    model::{self, editable_value::EditableValue, hex_color::HexColor},
    photo::Photo,
    photo_renderer::{PhotoRenderOptions, PhotoRenderStatus, PhotoRenderer},
    template::TemplateRegion,
    theme::color,
    utils::{IdExt, Toggle},
    widget::{
        canvas::CanvasPhoto,
        icon_button::IconButton,
        placeholder::RectPlaceholder,
        transformable::{TransformHandleMode, TransformableState},
    },
};

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
    pub color: EditableValue<HexColor>,
}

impl CanvasTextEditState {
    pub fn new(font_size: f32, color: Color32) -> Self {
        Self {
            font_size: EditableValue::new(font_size),
            color: EditableValue::new(HexColor(color)),
        }
    }

    pub fn update(&mut self, font_size: f32, color: Color32) {
        self.font_size.update_if_not_active(font_size);
        self.color.update_if_not_active(HexColor(color));
    }
}

#[derive(Debug, Clone, PartialEq, Display, EnumIter, Copy)]
pub enum TextHorizontalAlignment {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Display, EnumIter, Copy)]
pub enum TextVerticalAlignment {
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasText {
    pub text: String,
    pub font_size: f32,
    pub font_id: FontId,
    pub color: Color32,
    pub edit_state: CanvasTextEditState,
    pub horizontal_alignment: TextHorizontalAlignment,
    pub vertical_alignment: TextVerticalAlignment,
}

impl CanvasText {
    pub fn new(
        text: String,
        font_size: f32,
        font_family: FontId,
        color: Color32,
        horizontal_alignment: TextHorizontalAlignment,
        vertical_alignment: TextVerticalAlignment,
    ) -> Self {
        Self {
            text,
            font_size,
            font_id: font_family,
            edit_state: CanvasTextEditState::new(font_size, color),
            color,
            horizontal_alignment,
            vertical_alignment,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineSlope {
    Positive,
    Negative,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanvasShapeKind {
    Rectangle { corner_radius: f32 },
    Ellipse,
    Line { slope: LineSlope },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasShape {
    pub kind: CanvasShapeKind,
    pub fill_color: Color32,
    pub stroke: Option<(Stroke, StrokeKind)>,
    pub edit_state: CanvasShapeEditState,
}

impl CanvasShape {
    pub fn new(kind: CanvasShapeKind, color: Color32, _rect: Rect) -> Self {
        Self {
            kind,
            fill_color: color,
            stroke: None,
            edit_state: CanvasShapeEditState::default(),
        }
    }

    pub fn rectangle(color: Color32) -> Self {
        Self {
            kind: CanvasShapeKind::Rectangle { corner_radius: 0.0 },
            fill_color: color,
            stroke: None,
            edit_state: CanvasShapeEditState::default(),
        }
    }

    pub fn ellipse(color: Color32) -> Self {
        Self {
            kind: CanvasShapeKind::Ellipse,
            fill_color: color,
            stroke: None,
            edit_state: CanvasShapeEditState::default(),
        }
    }

    pub fn line(color: Color32, width: f32, start: Pos2, end: Pos2) -> Self {
        let slope = if end.y <= start.y {
            LineSlope::Positive
        } else {
            LineSlope::Negative
        };
        println!("Slope = {:?}", slope);
        Self {
            kind: CanvasShapeKind::Line { slope },
            fill_color: color,
            stroke: Some((Stroke::new(width, color), StrokeKind::Middle)),
            edit_state: CanvasShapeEditState::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasShapeEditState {
    pub stroke_width: EditableValue<f32>,
    pub fill_color: EditableValue<HexColor>,
    pub stroke_color: EditableValue<HexColor>,
}

impl CanvasShapeEditState {
    pub fn new(stroke_width: f32, fill_color: Color32, stroke_color: Color32) -> Self {
        Self {
            stroke_width: EditableValue::new(stroke_width),
            fill_color: EditableValue::new(HexColor(fill_color)),
            stroke_color: EditableValue::new(HexColor(stroke_color)),
        }
    }

    pub fn update(&mut self, stroke_width: f32, fill_color: Color32, stroke_color: Color32) {
        self.stroke_width.update_if_not_active(stroke_width);
        self.fill_color.update_if_not_active(HexColor(fill_color));
        self.stroke_color
            .update_if_not_active(HexColor(stroke_color));
    }
}

impl Default for CanvasShapeEditState {
    fn default() -> Self {
        Self {
            stroke_width: EditableValue::new(1.0),
            fill_color: EditableValue::new(HexColor(color::BLACK)),
            stroke_color: EditableValue::new(HexColor(color::BLACK)),
        }
    }
}

// Tool settings for pre-creation configuration
#[derive(Debug, Clone, PartialEq)]
pub struct TextToolSettings {
    pub font_size: f32,
    pub font_id: FontId,
    pub color: Color32,
    pub horizontal_alignment: TextHorizontalAlignment,
    pub vertical_alignment: TextVerticalAlignment,
    pub edit_state: CanvasTextEditState,
}

impl Default for TextToolSettings {
    fn default() -> Self {
        Self {
            font_size: 24.0,
            font_id: FontId::default(),
            color: color::BLACK,
            horizontal_alignment: TextHorizontalAlignment::Left,
            vertical_alignment: TextVerticalAlignment::Top,
            edit_state: CanvasTextEditState::new(24.0, color::BLACK),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapeToolSettings {
    pub fill_color: Color32,
    pub stroke: Option<(Stroke, StrokeKind)>,
    pub edit_state: CanvasShapeEditState,
}

impl Default for ShapeToolSettings {
    fn default() -> Self {
        Self {
            fill_color: color::BLUE_SOFT,
            stroke: None,
            edit_state: CanvasShapeEditState::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LineToolSettings {
    pub color: Color32,
    pub width: f32,
    pub edit_state: CanvasShapeEditState,
}

impl Default for LineToolSettings {
    fn default() -> Self {
        Self {
            color: color::BLACK,
            width: 2.0,
            edit_state: CanvasShapeEditState::default(),
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
        scale_mode: model::scale_mode::ScaleMode,
    },
    TemplateText {
        region: TemplateRegion,
        text: CanvasText,
    },
    Shape(CanvasShape),
}

impl LayerContent {
    pub fn is_photo(&self) -> bool {
        matches!(self, LayerContent::Photo(_)) || matches!(self, LayerContent::TemplatePhoto { .. })
    }

    pub fn is_text(&self) -> bool {
        matches!(self, LayerContent::Text(_)) || matches!(self, LayerContent::TemplateText { .. })
    }

    pub fn is_template(&self) -> bool {
        matches!(self, LayerContent::TemplatePhoto { .. })
            || matches!(self, LayerContent::TemplateText { .. })
    }

    pub fn is_shape(&self) -> bool {
        matches!(self, LayerContent::Shape(_))
    }
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
    /// Returns the document portion of the layer with editor-only state reset.
    ///
    /// History snapshots use this representation so newly added layer fields
    /// participate in equality automatically through `Layer::PartialEq`.
    pub(crate) fn persistent_clone(&self) -> Self {
        let mut layer = self.clone();
        layer.selected = false;
        layer.transform_state = layer.transform_state.persistent_clone();
        layer.transform_state.handle_mode = TransformHandleMode::default();
        layer.transform_edit_state = LayerTransformEditState::from(&layer.transform_state);

        match &mut layer.content {
            LayerContent::Text(text) | LayerContent::TemplateText { text, .. } => {
                text.edit_state = CanvasTextEditState::new(text.font_size, text.color);
            }
            LayerContent::Shape(shape) => {
                let (stroke_width, stroke_color) = shape
                    .stroke
                    .map(|(stroke, _)| (stroke.width, stroke.color))
                    .unwrap_or((1.0, color::BLACK));
                shape.edit_state =
                    CanvasShapeEditState::new(stroke_width, shape.fill_color, stroke_color);
            }
            LayerContent::Photo(_) | LayerContent::TemplatePhoto { .. } => {}
        }

        layer
    }

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
        Self::new_text_layer_with_settings(
            &TextToolSettings::default(),
            Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 100.0)),
        )
    }

    pub fn new_text_layer_with_settings(settings: &TextToolSettings, rect: Rect) -> Self {
        let text = CanvasText::new(
            String::new(),
            settings.font_size,
            settings.font_id.clone(),
            settings.color,
            settings.horizontal_alignment,
            settings.vertical_alignment,
        );
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
            content: LayerContent::Text(text),
            name: String::new(),
            visible: true,
            locked: false,
            selected: false,
            id: next_layer_id(),
            transform_edit_state,
            transform_state,
        }
    }

    pub fn new_rectangle_shape_layer() -> Self {
        Self::new_rectangle_shape_layer_with_settings(
            &ShapeToolSettings::default(),
            Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 100.0)),
        )
    }

    pub fn new_rectangle_shape_layer_with_settings(
        settings: &ShapeToolSettings,
        rect: Rect,
    ) -> Self {
        let mut shape = CanvasShape::rectangle(settings.fill_color);
        shape.stroke = settings.stroke;
        shape.edit_state = settings.edit_state.clone();
        let transform_state = TransformableState::new(rect);
        let transform_edit_state = LayerTransformEditState::from(&transform_state);
        Self {
            content: LayerContent::Shape(shape),
            name: "New Rectangle Shape Layer".to_string(),
            visible: true,
            locked: false,
            selected: false,
            id: next_layer_id(),
            transform_edit_state,
            transform_state,
        }
    }

    pub fn new_ellipse_shape_layer() -> Self {
        Self::new_ellipse_shape_layer_with_settings(
            &ShapeToolSettings::default(),
            Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 100.0)),
        )
    }

    pub fn new_ellipse_shape_layer_with_settings(settings: &ShapeToolSettings, rect: Rect) -> Self {
        let mut shape = CanvasShape::ellipse(settings.fill_color);
        shape.stroke = settings.stroke;
        shape.edit_state = settings.edit_state.clone();
        let transform_state = TransformableState::new(rect);
        let transform_edit_state = LayerTransformEditState::from(&transform_state);
        Self {
            content: LayerContent::Shape(shape),
            name: "New Ellipse Shape Layer".to_string(),
            visible: true,
            locked: false,
            selected: false,
            id: next_layer_id(),
            transform_edit_state,
            transform_state,
        }
    }

    pub fn new_line_shape_layer_with_settings(
        settings: &LineToolSettings,
        start_pos: Pos2,
        end_pos: Pos2,
    ) -> Self {
        let mut shape = CanvasShape::line(settings.color, settings.width, start_pos, end_pos);
        shape.stroke = Some((
            Stroke::new(settings.width, settings.color),
            StrokeKind::Middle,
        ));
        shape.edit_state = settings.edit_state.clone();
        let transform_state = TransformableState::new(Rect::from_two_pos(start_pos, end_pos));
        let transform_edit_state = LayerTransformEditState::from(&transform_state);
        Self {
            content: LayerContent::Shape(shape),
            name: "Line".to_string(),
            visible: true,
            locked: false,
            selected: false,
            id: next_layer_id(),
            transform_edit_state,
            transform_state,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayersResponse {
    SelectedLayer(LayerId),
    Changed,
    None,
}

#[derive(Debug)]
pub struct Layers<'a> {
    layers: &'a mut IndexMap<LayerId, Layer>,
}

impl Hash for Layer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<'a> Layers<'a> {
    const ROW_HEIGHT: f32 = 50.0;
    const VISIBILITY_WIDTH: f32 = 34.0;
    const THUMBNAIL_SIZE: f32 = 34.0;
    const LOCK_WIDTH: f32 = 32.0;

    pub fn new(layers: &'a mut IndexMap<LayerId, Layer>) -> Self {
        Self { layers }
    }

    pub fn show(&mut self, ui: &mut eframe::egui::Ui) -> LayersResponse {
        let mut selected_layer_id = None;
        let mut from = None;
        let mut to = None;
        let mut changed = false;

        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (idx, (layer_id, layer)) in self.layers.iter_mut().rev().enumerate() {
                let row_response = Self::show_layer_row(ui, *layer_id, layer);
                if !layer.locked {
                    row_response.response.dnd_set_drag_payload(*layer_id);
                }

                if row_response.changed {
                    changed = true;
                }
                if row_response.row_clicked {
                    selected_layer_id = Some(*layer_id);
                }

                if let (Some(pointer), Some(hovered_idx)) = (
                    ui.input(|i| i.pointer.interact_pos()),
                    row_response.response.dnd_hover_payload::<LayerId>(),
                ) {
                    let rect = row_response.response.rect;
                    let stroke = egui::Stroke::new(2.0, color::ACCENT);

                    let line_y = if *hovered_idx == *layer_id {
                        None
                    } else if pointer.y < rect.center().y {
                        Some(rect.top())
                    } else {
                        Some(rect.bottom())
                    };

                    if let Some(line_y) = line_y {
                        ui.painter().hline(rect.x_range(), line_y, stroke);
                        to = Some(if line_y == rect.bottom() {
                            idx + 1
                        } else {
                            idx
                        });
                    }

                    if let Some(dragged_id) = row_response.response.dnd_release_payload::<LayerId>()
                    {
                        from = Some(*dragged_id);
                    }
                }
            }
        });

        let from_idx = from.and_then(|from_id| {
            self.layers
                .keys()
                .rev()
                .position(|layer_id| *layer_id == from_id)
        });
        if let (Some(from_idx), Some(to_idx)) = (from_idx, to)
            && Self::reorder_display_layers(self.layers, from_idx, to_idx)
        {
            changed = true;
        }

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

        match (selected_layer_id, changed) {
            (Some(selected_layer_id), _) => LayersResponse::SelectedLayer(selected_layer_id),
            (None, true) => LayersResponse::Changed,
            (None, false) => LayersResponse::None,
        }
    }

    fn reorder_display_layers(
        layers: &mut IndexMap<LayerId, Layer>,
        from_idx: usize,
        to_boundary_idx: usize,
    ) -> bool {
        let len = layers.len();
        if from_idx >= len || to_boundary_idx > len {
            return false;
        }

        // The drop target is a boundary in the pre-removal display order. Removing an item
        // above that boundary shifts a downward destination up by one.
        let target_idx = if from_idx < to_boundary_idx {
            to_boundary_idx.saturating_sub(1)
        } else {
            to_boundary_idx
        };
        if target_idx == from_idx || target_idx >= len {
            return false;
        }

        let mut display_layers = layers.clone().into_iter().rev().collect::<IndexMap<_, _>>();
        let (from_key, from_layer) = {
            let (from_key, from_layer) = display_layers.get_index(from_idx).unwrap();
            (*from_key, from_layer.clone())
        };
        display_layers.shift_insert(target_idx, from_key, from_layer);
        *layers = display_layers.into_iter().rev().collect();
        true
    }

    fn show_layer_row(ui: &mut egui::Ui, layer_id: LayerId, layer: &mut Layer) -> LayerRowResponse {
        let size = Vec2::new(ui.available_width().max(0.0), Self::ROW_HEIGHT);
        let (_, rect) = ui.allocate_space(size);
        let row_id = ui.make_persistent_id(("layer_row", layer_id));
        let sense = if layer.locked {
            Sense::click()
        } else {
            Sense::click_and_drag()
        };
        let mut row_response = ui.interact(rect, row_id, sense);
        row_response.set_intrinsic_size(size);

        let visibility_width = Self::VISIBILITY_WIDTH.min(rect.width() * 0.5);
        let lock_width = Self::LOCK_WIDTH.min((rect.width() - visibility_width).max(0.0));
        let visibility_rect =
            Rect::from_min_size(rect.min, Vec2::new(visibility_width, Self::ROW_HEIGHT));
        let lock_rect = Rect::from_min_size(
            Pos2::new(rect.right() - lock_width, rect.top()),
            Vec2::new(lock_width, Self::ROW_HEIGHT),
        );

        let painter = ui.painter_at(rect);
        let row_background = painter.add(egui::Shape::Noop);
        let visibility_background = painter.add(egui::Shape::Noop);
        let lock_background = painter.add(egui::Shape::Noop);

        let visibility_response = ui
            .push_id(row_id.with("visibility"), |ui| {
                ui.place(
                    visibility_rect,
                    IconButton::new(Asset::material_visibility_off())
                        .tint(color::SURFACE_STRONG)
                        .active(layer.visible)
                        .tint_active(color::CONTROL_TEXT)
                        .icon_active(Asset::material_visibility())
                        .size(visibility_rect.size())
                        .icon_size(Vec2::splat(18.0)),
                )
            })
            .inner
            .on_hover_text(if layer.visible {
                "Hide layer"
            } else {
                "Show layer"
            });
        let lock_response = ui
            .push_id(row_id.with("lock"), |ui| {
                ui.place(
                    lock_rect,
                    IconButton::new(Asset::material_lock_open())
                        .tint(color::SURFACE_STRONG)
                        .active(layer.locked)
                        .tint_active(color::CONTROL_TEXT)
                        .icon_active(Asset::material_lock())
                        .size(lock_rect.size())
                        .icon_size(Vec2::splat(17.0)),
                )
            })
            .inner
            .on_hover_text(if layer.locked {
                "Unlock layer"
            } else {
                "Lock layer"
            });

        let visibility_clicked = visibility_response.clicked();
        let lock_clicked = lock_response.clicked();
        if visibility_clicked {
            layer.visible.toggle();
            if !layer.visible {
                layer.selected = false;
            }
        }
        if lock_clicked {
            layer.locked.toggle();
        }

        let row_clicked = row_response.clicked() && !visibility_clicked && !lock_clicked;
        let ctrl = ui.input(|input| input.modifiers.ctrl);
        let selected_for_response = if row_clicked {
            if ctrl { !layer.selected } else { true }
        } else {
            layer.selected
        };
        let name = Self::display_name(layer);

        let fill = if selected_for_response {
            color::ACCENT_MUTED
        } else if row_response.hovered() || visibility_response.hovered() || lock_response.hovered()
        {
            color::SURFACE_DARK
        } else {
            color::SIDE_PANEL_BACKGROUND
        };
        painter.set(row_background, egui::Shape::rect_filled(rect, 0.0, fill));
        if selected_for_response {
            painter.rect_filled(
                Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())),
                0.0,
                color::ACCENT,
            );
        }
        painter.hline(
            rect.x_range(),
            rect.bottom(),
            egui::Stroke::new(1.0, color::SURFACE_DARK),
        );

        if visibility_response.hovered() {
            painter.set(
                visibility_background,
                egui::Shape::rect_filled(visibility_rect.shrink(5.0), 4.0, color::SURFACE_MUTED),
            );
        }

        let thumbnail_left = visibility_rect.right() + 6.0;
        let thumbnail_space = lock_rect.left() - 5.0 - thumbnail_left;
        let thumbnail_rect = (thumbnail_space >= Self::THUMBNAIL_SIZE + 8.0).then(|| {
            Rect::from_min_size(
                Pos2::new(thumbnail_left, rect.center().y - Self::THUMBNAIL_SIZE / 2.0),
                Vec2::splat(Self::THUMBNAIL_SIZE),
            )
        });
        if let Some(thumbnail_rect) = thumbnail_rect {
            Self::paint_thumbnail(ui, layer_id, layer, thumbnail_rect);
        }

        let text_left =
            thumbnail_rect.map_or(visibility_rect.right() + 5.0, |rect| rect.right() + 9.0);
        let text_right = lock_rect.left() - 5.0;
        let mut name_was_elided = true;
        if text_right - text_left >= 8.0 {
            let name_rect = Rect::from_min_max(
                Pos2::new(text_left, rect.top() + 8.0),
                Pos2::new(text_right, rect.top() + 28.0),
            );
            let kind_rect = Rect::from_min_max(
                Pos2::new(text_left, rect.top() + 27.0),
                Pos2::new(text_right, rect.bottom() - 5.0),
            );
            let primary_text = if layer.visible {
                color::WHITE
            } else {
                color::SURFACE_EMPHASIS
            };
            let mut name_job = LayoutJob::single_section(
                name.to_owned(),
                TextFormat {
                    font_id: FontId::proportional(13.0),
                    color: primary_text,
                    ..Default::default()
                },
            );
            name_job.wrap = TextWrapping::truncate_at_width(name_rect.width());
            let name_galley = painter.layout_job(name_job);
            name_was_elided = name_galley.elided;
            painter.galley(name_rect.left_top(), name_galley, primary_text);

            let mut kind_job = LayoutJob::single_section(
                Self::content_kind(&layer.content).to_owned(),
                TextFormat {
                    font_id: FontId::proportional(10.5),
                    color: color::SURFACE_EMPHASIS,
                    ..Default::default()
                },
            );
            kind_job.wrap = TextWrapping::truncate_at_width(kind_rect.width());
            painter.galley(
                kind_rect.left_top(),
                painter.layout_job(kind_job),
                color::SURFACE_EMPHASIS,
            );
        }
        if name_was_elided {
            row_response.clone().on_hover_text(name);
        }

        if lock_response.hovered() {
            painter.set(
                lock_background,
                egui::Shape::rect_filled(lock_rect.shrink(5.0), 4.0, color::SURFACE_MUTED),
            );
        }

        LayerRowResponse {
            response: row_response,
            row_clicked,
            changed: visibility_clicked || lock_clicked,
        }
    }

    fn display_name(layer: &Layer) -> &str {
        if !layer.name.trim().is_empty() {
            return &layer.name;
        }

        match &layer.content {
            LayerContent::Text(text) if !text.text.trim().is_empty() => &text.text,
            LayerContent::Text(_) => "Text layer",
            LayerContent::TemplateText { text, .. } if !text.text.trim().is_empty() => &text.text,
            LayerContent::TemplateText { .. } => "Template text",
            LayerContent::TemplatePhoto { .. } => "Template photo",
            LayerContent::Photo(_) => "Photo",
            LayerContent::Shape(shape) => match shape.kind {
                CanvasShapeKind::Rectangle { .. } => "Rectangle",
                CanvasShapeKind::Ellipse => "Ellipse",
                CanvasShapeKind::Line { .. } => "Line",
            },
        }
    }

    fn content_kind(content: &LayerContent) -> &'static str {
        match content {
            LayerContent::Photo(_) => "PHOTO",
            LayerContent::Text(_) => "TEXT",
            LayerContent::TemplatePhoto { .. } => "TEMPLATE PHOTO",
            LayerContent::TemplateText { .. } => "TEMPLATE TEXT",
            LayerContent::Shape(shape) => match shape.kind {
                CanvasShapeKind::Rectangle { .. } => "RECTANGLE",
                CanvasShapeKind::Ellipse => "ELLIPSE",
                CanvasShapeKind::Line { .. } => "LINE",
            },
        }
    }

    fn paint_thumbnail(ui: &mut egui::Ui, layer_id: LayerId, layer: &Layer, rect: Rect) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 3.0, color::SURFACE);
        painter.rect_stroke(
            rect,
            3.0,
            egui::Stroke::new(1.0, color::SURFACE_MUTED),
            StrokeKind::Inside,
        );

        match &layer.content {
            LayerContent::Photo(canvas_photo) => {
                Self::show_photo_thumbnail(ui, layer_id, canvas_photo, rect.shrink(2.0));
            }
            LayerContent::TemplatePhoto {
                photo: Some(canvas_photo),
                ..
            } => {
                Self::show_photo_thumbnail(ui, layer_id, canvas_photo, rect.shrink(2.0));
                Self::paint_template_badge(&painter, rect);
            }
            LayerContent::TemplatePhoto { photo: None, .. } => {
                let image_rect = rect.shrink(7.0);
                painter.rect_stroke(
                    image_rect,
                    1.0,
                    egui::Stroke::new(1.5, color::CONTROL_TEXT),
                    StrokeKind::Inside,
                );
                painter.line_segment(
                    [image_rect.left_bottom(), image_rect.center()],
                    egui::Stroke::new(1.5, color::CONTROL_TEXT),
                );
                painter.line_segment(
                    [image_rect.center(), image_rect.right_bottom()],
                    egui::Stroke::new(1.5, color::CONTROL_TEXT),
                );
                Self::paint_template_badge(&painter, rect);
            }
            LayerContent::Text(text) | LayerContent::TemplateText { text, .. } => {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "T",
                    FontId::proportional(20.0),
                    text.color,
                );
                if matches!(layer.content, LayerContent::TemplateText { .. }) {
                    Self::paint_template_badge(&painter, rect);
                }
            }
            LayerContent::Shape(shape) => {
                let icon_rect = rect.shrink(8.0);
                match shape.kind {
                    CanvasShapeKind::Rectangle { .. } => {
                        painter.rect_filled(icon_rect, 2.0, shape.fill_color);
                        painter.rect_stroke(
                            icon_rect,
                            2.0,
                            egui::Stroke::new(1.0, color::CONTROL_TEXT),
                            StrokeKind::Inside,
                        );
                    }
                    CanvasShapeKind::Ellipse => {
                        painter.circle_filled(
                            icon_rect.center(),
                            icon_rect.width() / 2.0,
                            shape.fill_color,
                        );
                        painter.circle_stroke(
                            icon_rect.center(),
                            icon_rect.width() / 2.0,
                            egui::Stroke::new(1.0, color::CONTROL_TEXT),
                        );
                    }
                    CanvasShapeKind::Line {
                        slope: LineSlope::Positive,
                    } => {
                        painter.line_segment(
                            [icon_rect.left_bottom(), icon_rect.right_top()],
                            egui::Stroke::new(2.0, shape.fill_color),
                        );
                    }
                    CanvasShapeKind::Line {
                        slope: LineSlope::Negative,
                    } => {
                        painter.line_segment(
                            [icon_rect.left_top(), icon_rect.right_bottom()],
                            egui::Stroke::new(2.0, shape.fill_color),
                        );
                    }
                }
            }
        }
    }

    fn paint_template_badge(painter: &egui::Painter, rect: Rect) {
        let badge = Rect::from_min_size(
            Pos2::new(rect.right() - 10.0, rect.top() + 2.0),
            Vec2::splat(8.0),
        );
        painter.rect_filled(badge, CornerRadius::same(2), color::ACCENT);
        painter.text(
            badge.center(),
            Align2::CENTER_CENTER,
            "T",
            FontId::proportional(6.5),
            color::WHITE,
        );
    }

    fn show_photo_thumbnail(
        ui: &mut egui::Ui,
        layer_id: LayerId,
        canvas_photo: &CanvasPhoto,
        bounds: Rect,
    ) {
        let image_size = Self::cropped_thumbnail_size(canvas_photo, bounds.size());
        let image_rect = Rect::from_center_size(bounds.center(), image_size);

        let render_key = format!("layer-thumbnail:{layer_id}");
        let render_status = PhotoRenderer::paint(
            ui,
            &canvas_photo.photo,
            &canvas_photo.adjustments,
            image_rect,
            PhotoRenderOptions::default()
                .thumbnail()
                .with_crop(canvas_photo.crop)
                .with_render_key(&render_key),
        );

        if !matches!(
            render_status,
            Ok(PhotoRenderStatus::Ready
                | PhotoRenderStatus::Placeholder
                | PhotoRenderStatus::NotVisible)
        ) {
            ui.place(
                image_rect,
                RectPlaceholder::new(image_size, color::SURFACE_EMPHASIS, 0.0),
            );
        }
    }

    fn cropped_thumbnail_size(canvas_photo: &CanvasPhoto, bounds: Vec2) -> Vec2 {
        let cropped_size = Vec2::new(
            canvas_photo.photo.metadata.rotated_width() as f32 * canvas_photo.crop.width(),
            canvas_photo.photo.metadata.rotated_height() as f32 * canvas_photo.crop.height(),
        );
        if cropped_size.x <= 0.0 || cropped_size.y <= 0.0 {
            return bounds;
        }

        cropped_size * (bounds.x / cropped_size.x).min(bounds.y / cropped_size.y)
    }
}

#[derive(Debug)]
struct LayerRowResponse {
    response: Response,
    row_clicked: bool,
    changed: bool,
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;

    use super::*;

    fn test_layers() -> (IndexMap<LayerId, Layer>, LayerId, LayerId) {
        let bottom = Layer::new_rectangle_shape_layer();
        let bottom_id = bottom.id;
        let top = Layer::new_ellipse_shape_layer();
        let top_id = top.id;

        (
            IndexMap::from([(bottom_id, bottom), (top_id, top)]),
            bottom_id,
            top_id,
        )
    }

    fn harness_for(layers: IndexMap<LayerId, Layer>) -> Harness<'static, IndexMap<LayerId, Layer>> {
        harness_for_width(layers, 300.0)
    }

    fn harness_for_width(
        layers: IndexMap<LayerId, Layer>,
        width: f32,
    ) -> Harness<'static, IndexMap<LayerId, Layer>> {
        Harness::builder()
            .with_size(Vec2::new(width + 40.0, 180.0))
            .build_ui_state(
                move |ui, layers| {
                    ui.set_width(width);
                    Layers::new(layers).show(ui);
                },
                layers,
            )
    }

    fn click_at(harness: &mut Harness<IndexMap<LayerId, Layer>>, pos: Pos2) {
        harness.drag_at(pos);
        harness.drop_at(pos);
        harness.run();
    }

    #[test]
    fn row_and_layer_controls_remain_clickable() {
        let (layers, bottom_id, top_id) = test_layers();
        let mut harness = harness_for(layers);

        // Rows are displayed in reverse stack order, so the top layer is the first row.
        click_at(&mut harness, Pos2::new(180.0, 30.0));
        assert!(harness.state().get(&top_id).unwrap().selected);
        assert!(!harness.state().get(&bottom_id).unwrap().selected);

        click_at(&mut harness, Pos2::new(24.0, 30.0));
        assert!(!harness.state().get(&top_id).unwrap().visible);
        assert!(!harness.state().get(&top_id).unwrap().selected);

        click_at(&mut harness, Pos2::new(292.0, 30.0));
        assert!(harness.state().get(&top_id).unwrap().locked);
    }

    #[test]
    fn dragging_a_row_reorders_layers() {
        let (layers, bottom_id, top_id) = test_layers();
        let mut harness = harness_for(layers);

        harness.drag_at(Pos2::new(180.0, 30.0));
        harness.run();
        harness.hover_at(Pos2::new(180.0, 90.0));
        harness.run();
        harness.drop_at(Pos2::new(180.0, 90.0));
        harness.run();

        assert_eq!(
            harness.state().keys().copied().collect::<Vec<_>>(),
            vec![top_id, bottom_id]
        );
    }

    #[test]
    fn locked_row_cannot_be_reordered() {
        let (mut layers, bottom_id, top_id) = test_layers();
        layers.get_mut(&top_id).unwrap().locked = true;
        let mut harness = harness_for(layers);

        harness.drag_at(Pos2::new(180.0, 30.0));
        harness.run();
        harness.hover_at(Pos2::new(180.0, 90.0));
        harness.run();
        harness.drop_at(Pos2::new(180.0, 90.0));
        harness.run();

        assert_eq!(
            harness.state().keys().copied().collect::<Vec<_>>(),
            vec![bottom_id, top_id]
        );
    }

    #[test]
    fn moving_a_display_row_down_accounts_for_the_removed_row() {
        let (mut layers, bottom_id, top_id) = test_layers();
        let mut middle = Layer::new_rectangle_shape_layer();
        middle.name = "Middle".to_owned();
        let middle_id = middle.id;
        layers.shift_insert(1, middle_id, middle);

        // Display order starts as Top, Middle, Bottom. Move Top to the boundary after Middle.
        assert!(Layers::reorder_display_layers(&mut layers, 0, 2));

        assert_eq!(
            layers.keys().copied().collect::<Vec<_>>(),
            vec![bottom_id, top_id, middle_id]
        );
    }

    #[test]
    fn narrow_rows_keep_visibility_and_lock_controls_inside_the_row() {
        let (layers, _bottom_id, top_id) = test_layers();
        let mut harness = harness_for_width(layers, 80.0);

        click_at(&mut harness, Pos2::new(24.0, 30.0));
        assert!(!harness.state().get(&top_id).unwrap().visible);

        click_at(&mut harness, Pos2::new(72.0, 30.0));
        assert!(harness.state().get(&top_id).unwrap().locked);
    }
}
