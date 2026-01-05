use eframe::{
    egui::{self, FontId, Id},
    epaint::{Color32, Pos2, Rect, Vec2},
};
use indexmap::{IndexMap, indexmap};

use crate::{
    dependencies::{Dependency, SingletonFor},
    id::{LayerId, next_layer_id},
    model::{edit_state::EditablePage, page::Page, scale_mode::ScaleMode},
    photo::Photo,
    project_settings::ProjectSettingsManager,
    template::{Template, TemplateRegionKind},
    utils::{IdExt, RectExt},
    widget::{
        canvas::types::{IdleTool, ToolState},
        canvas_info::{
            layers::{
                CanvasText, Layer, LayerContent, LayerTransformEditState, LineToolSettings,
                ShapeToolSettings, TextHorizontalAlignment, TextToolSettings,
                TextVerticalAlignment,
            },
            quick_layout::{self},
        },
        transformable::{ResizeMode, TransformHandle, TransformHandleMode, TransformableState},
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
    pub last_quick_layout: Option<quick_layout::Layout>,
    pub canvas_id: egui::Id,
    pub text_edit_mode: Option<LayerId>,
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
            text_edit_mode: None,
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
            quick_layout_order: quick_layout_order,
            last_quick_layout: None,
            canvas_id: Id::random(),
            text_edit_mode: None,
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
            text_edit_mode: None,
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
            text_edit_mode: None,
            tool_state: ToolState::Idle(IdleTool::Select),
            text_tool_settings: TextToolSettings::default(),
            rectangle_tool_settings: ShapeToolSettings::default(),
            ellipse_tool_settings: ShapeToolSettings::default(),
            line_tool_settings: LineToolSettings::default(),
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

    pub fn is_layer_selected(&self, layer_id: &LayerId) -> bool {
        self.layers.get(layer_id).unwrap().selected
    }

    pub fn selected_layers_iter_mut(&mut self) -> impl Iterator<Item = &mut Layer> {
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
            if !layer.1.content.is_photo() {
                continue;
            }
            if !self.quick_layout_order.contains(&layer.0) {
                self.quick_layout_order.push(*layer.0);
            }
        }
    }
}
