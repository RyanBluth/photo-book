use eframe::{
    egui::{self, FontId, Id},
    epaint::{Pos2, Rect, Vec2},
};
use indexmap::{IndexMap, indexmap};

use crate::{
    dep,
    id::{LayerId, next_layer_id},
    layout::{LayoutItem, LayoutNode},
    model::{edit_state::EditablePage, page::Page, scale_mode::ScaleMode},
    photo::Photo,
    project_settings::ProjectSettingsManager,
    template::{Template, TemplateRegionKind},
    theme::color,
    utils::{IdExt, RectExt},
    widget::{
        canvas::types::{IdleTool, ToolState},
        canvas_info::layers::{
            CanvasText, Layer, LayerContent, LayerTransformEditState, LineToolSettings,
            ShapeToolSettings, TextHorizontalAlignment, TextToolSettings, TextVerticalAlignment,
        },
        transformable::{TransformHandleMode, TransformableState},
    },
};

use super::{selection::MultiSelect, types::CanvasPhoto};

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasState {
    pub layers: IndexMap<LayerId, Layer>,
    pub zoom: f32,
    pub offset: Vec2,
    pub multi_select: Option<MultiSelect>,
    pub page: EditablePage,
    pub template: Option<Template>,
    pub quick_layout_order: Vec<LayerId>,
    pub last_quick_layout: Option<LayoutNode>,
    pub canvas_id: egui::Id,
    pub text_edit_mode: TextEditMode,
    pub tool_state: ToolState,
    pub text_tool_settings: TextToolSettings,
    pub rectangle_tool_settings: ShapeToolSettings,
    pub ellipse_tool_settings: ShapeToolSettings,
    pub line_tool_settings: LineToolSettings,
    pub computed_initial_zoom: bool,
}

impl CanvasState {
    pub fn new() -> Self {
        Self {
            layers: IndexMap::new(),
            zoom: 1.0,
            offset: Vec2::ZERO,
            multi_select: None,
            page: EditablePage::new(dep!(ProjectSettingsManager, |manager| {
                manager
                    .project_settings
                    .default_page
                    .clone()
                    .unwrap_or_default()
            })),
            template: None,
            quick_layout_order: Vec::new(),
            last_quick_layout: None,
            canvas_id: Id::random(),
            text_edit_mode: TextEditMode::None,
            tool_state: ToolState::Idle(IdleTool::Select),
            text_tool_settings: TextToolSettings::default(),
            rectangle_tool_settings: ShapeToolSettings::default(),
            ellipse_tool_settings: ShapeToolSettings::default(),
            line_tool_settings: LineToolSettings::default(),
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
            quick_layout_order,
            last_quick_layout: None,
            canvas_id: Id::random(),
            text_edit_mode: TextEditMode::None,
            tool_state: ToolState::Idle(IdleTool::Select),
            text_tool_settings: TextToolSettings::default(),
            rectangle_tool_settings: ShapeToolSettings::default(),
            ellipse_tool_settings: ShapeToolSettings::default(),
            line_tool_settings: LineToolSettings::default(),
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
            text_edit_mode: TextEditMode::None,
            tool_state: ToolState::Idle(IdleTool::Select),
            text_tool_settings: TextToolSettings::default(),
            rectangle_tool_settings: ShapeToolSettings::default(),
            ellipse_tool_settings: ShapeToolSettings::default(),
            line_tool_settings: LineToolSettings::default(),
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
                                color::BLACK,
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
            text_edit_mode: TextEditMode::None,
            tool_state: ToolState::Idle(IdleTool::Select),
            text_tool_settings: TextToolSettings::default(),
            rectangle_tool_settings: ShapeToolSettings::default(),
            ellipse_tool_settings: ShapeToolSettings::default(),
            line_tool_settings: LineToolSettings::default(),
            computed_initial_zoom: false,
        }
    }

    pub fn swap_layer_centers_and_bounds(&mut self, layer_id1: LayerId, layer_id2: LayerId) {
        let original_child_a_rect = self.layers.get(&layer_id1).unwrap().transform_state.rect;

        let original_child_b_rect = self.layers.get(&layer_id2).unwrap().transform_state.rect;

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

    pub fn is_layer_selected(&self, layer_id: &LayerId) -> bool {
        self.layers.get(layer_id).unwrap().selected
    }

    pub fn is_layer_editable(layer: &Layer) -> bool {
        !layer.locked
    }

    pub fn is_layer_canvas_selectable(layer: &Layer) -> bool {
        layer.visible && Self::is_layer_editable(layer)
    }

    pub fn is_selected_layer_editable(layer: &Layer) -> bool {
        layer.selected && Self::is_layer_canvas_selectable(layer)
    }

    pub fn editable_layer(&self, layer_id: &LayerId) -> Option<&Layer> {
        self.layers
            .get(layer_id)
            .filter(|layer| Self::is_layer_canvas_selectable(layer))
    }

    pub fn editable_layer_mut(&mut self, layer_id: &LayerId) -> Option<&mut Layer> {
        self.layers
            .get_mut(layer_id)
            .filter(|layer| Self::is_layer_canvas_selectable(layer))
    }

    pub fn are_layers_canvas_editable(&self, layer_ids: &[LayerId]) -> bool {
        layer_ids
            .iter()
            .all(|layer_id| self.editable_layer(layer_id).is_some())
    }

    pub fn selected_editable_layers_iter(&self) -> impl Iterator<Item = &Layer> {
        self.layers
            .values()
            .filter(|layer| Self::is_selected_layer_editable(layer))
    }

    pub fn selected_layers_iter_mut(&mut self) -> impl Iterator<Item = &mut Layer> {
        self.layers
            .values_mut()
            .filter(|layer| Self::is_selected_layer_editable(layer))
    }

    pub fn selected_editable_layer_ids(&self) -> Vec<LayerId> {
        self.layers
            .iter()
            .filter(|(_, layer)| Self::is_selected_layer_editable(layer))
            .map(|(layer_id, _)| *layer_id)
            .collect()
    }

    pub fn select_layers_intersecting(&mut self, selection_box: Rect) {
        for layer in self.layers.values_mut() {
            layer.selected = Self::is_layer_canvas_selectable(layer)
                && layer.transform_state.rect.intersects(selection_box);
        }
    }

    pub fn delete_selected_editable_layers(&mut self) -> bool {
        let original_len = self.layers.len();
        self.layers
            .retain(|_, layer| !Self::is_selected_layer_editable(layer));
        let deleted = self.layers.len() != original_len;
        if deleted {
            self.update_quick_layout_order();
        }
        deleted
    }

    pub fn replace_selected_template_photo(&mut self, photo: Photo) -> bool {
        let selected_layer_ids = self
            .layers
            .iter()
            .filter(|(_, layer)| {
                Self::is_selected_layer_editable(layer)
                    && matches!(layer.content, LayerContent::TemplatePhoto { .. })
            })
            .map(|(layer_id, _)| *layer_id)
            .collect::<Vec<_>>();

        let [layer_id] = selected_layer_ids.as_slice() else {
            return false;
        };
        let Some(layer) = self.editable_layer_mut(layer_id) else {
            return false;
        };
        let LayerContent::TemplatePhoto {
            photo: canvas_photo,
            ..
        } = &mut layer.content
        else {
            return false;
        };

        *canvas_photo = Some(CanvasPhoto::new(photo));
        true
    }

    pub fn add_photo(&mut self, photo: Photo) {
        let layer = Layer::with_photo(photo);
        self.layers.insert(layer.id, layer);
        self.update_quick_layout_order();
    }

    pub fn update_quick_layout_order(&mut self) {
        self.quick_layout_order.retain(|id| {
            self.layers
                .get(id)
                .is_some_and(|layer| Self::quick_layout_item_for_layer(*id, layer).is_some())
        });

        for layer in &self.layers {
            if Self::quick_layout_item_for_layer(*layer.0, layer.1).is_some()
                && !self.quick_layout_order.contains(layer.0)
            {
                self.quick_layout_order.push(*layer.0);
            }
        }
    }

    pub fn quick_layout_items(&self) -> Vec<LayoutItem> {
        self.quick_layout_order
            .iter()
            .filter_map(|layer_id| {
                let layer = self.layers.get(layer_id)?;
                Self::quick_layout_item_for_layer(*layer_id, layer)
            })
            .collect()
    }

    fn quick_layout_item_for_layer(layer_id: LayerId, layer: &Layer) -> Option<LayoutItem> {
        let LayerContent::Photo(photo) = &layer.content else {
            return None;
        };

        let cropped_width = photo.photo.metadata.rotated_width() as f32 * photo.crop.width();
        let cropped_height = photo.photo.metadata.rotated_height() as f32 * photo.crop.height();
        Some(LayoutItem {
            aspect_ratio: cropped_width / cropped_height,
            id: layer_id,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextEditMode {
    None,
    BeginEditing(LayerId),
    Editing(LayerId),
}

impl TextEditMode {
    pub fn is_editing(&self, layer_id: &LayerId) -> bool {
        match self {
            TextEditMode::None => false,
            TextEditMode::BeginEditing(id) | TextEditMode::Editing(id) => id == layer_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::photo::{MetadataCollection, PhotoMetadata, PhotoMetadataField};

    fn shape_layer(rect: Rect, selected: bool, visible: bool, locked: bool) -> Layer {
        let mut layer =
            Layer::new_rectangle_shape_layer_with_settings(&ShapeToolSettings::default(), rect);
        layer.selected = selected;
        layer.visible = visible;
        layer.locked = locked;
        layer
    }

    fn state_with_layers(layers: Vec<Layer>) -> CanvasState {
        CanvasState::with_layers(
            layers.into_iter().map(|layer| (layer.id, layer)).collect(),
            EditablePage::new(Page::default()),
            None,
            Vec::new(),
        )
    }

    fn test_photo(name: &str) -> Photo {
        let mut fields = MetadataCollection::new();
        fields.insert(PhotoMetadataField::Width(400));
        fields.insert(PhotoMetadataField::Height(300));
        fields.insert(PhotoMetadataField::RotatedWidth(400));
        fields.insert(PhotoMetadataField::RotatedHeight(300));
        Photo::with_metadata(PathBuf::from(name), PhotoMetadata { fields })
    }

    #[test]
    fn marquee_selects_only_visible_unlocked_layers() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.0));
        let editable = shape_layer(rect, false, true, false);
        let hidden = shape_layer(rect, true, false, false);
        let locked = shape_layer(rect, true, true, true);
        let editable_id = editable.id;
        let hidden_id = hidden.id;
        let locked_id = locked.id;
        let mut state = state_with_layers(vec![editable, hidden, locked]);

        state.select_layers_intersecting(rect);

        assert!(state.layers[&editable_id].selected);
        assert!(!state.layers[&hidden_id].selected);
        assert!(!state.layers[&locked_id].selected);
    }

    #[test]
    fn deleting_selection_preserves_hidden_and_locked_layers() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.0));
        let editable = shape_layer(rect, true, true, false);
        let hidden = shape_layer(rect, true, false, false);
        let locked = shape_layer(rect, true, true, true);
        let unselected = shape_layer(rect, false, true, false);
        let editable_id = editable.id;
        let hidden_id = hidden.id;
        let locked_id = locked.id;
        let unselected_id = unselected.id;
        let mut state = state_with_layers(vec![editable, hidden, locked, unselected]);

        assert!(state.delete_selected_editable_layers());

        assert!(!state.layers.contains_key(&editable_id));
        assert!(state.layers.contains_key(&hidden_id));
        assert!(state.layers.contains_key(&locked_id));
        assert!(state.layers.contains_key(&unselected_id));
        assert!(state.editable_layer(&hidden_id).is_none());
        assert!(state.editable_layer(&locked_id).is_none());
    }

    #[test]
    fn template_photo_replacement_requires_an_editable_selection() {
        let mut state = CanvasState::with_template(crate::template::BUILT_IN[0].clone());
        let layer_id = *state.layers.keys().next().unwrap();
        let layer = state.layers.get_mut(&layer_id).unwrap();
        layer.selected = true;
        layer.locked = true;

        assert!(!state.replace_selected_template_photo(test_photo("blocked.jpg")));
        assert!(matches!(
            &state.layers[&layer_id].content,
            LayerContent::TemplatePhoto { photo: None, .. }
        ));

        state.layers.get_mut(&layer_id).unwrap().locked = false;
        assert!(state.replace_selected_template_photo(test_photo("replacement.jpg")));
        assert!(matches!(
            &state.layers[&layer_id].content,
            LayerContent::TemplatePhoto { photo: Some(_), .. }
        ));
    }
}
