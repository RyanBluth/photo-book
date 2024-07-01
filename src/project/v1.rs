use std::path::PathBuf;

use egui::{Color32, FontId, Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use crate::{
    model::scale_mode::ScaleMode as AppScaleMode, model::unit::Unit as AppUnit,
    photo_manager::PhotoManager, scene::SceneManager,
    template::TemplateRegionKind as AppTemplateRegionKind,
    widget::canvas_info::layers::LayerContent as AppLayerContent,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub photos: Vec<Photo>,
    pub pages: Vec<CanvasPage>,
}

impl Project {
    pub fn save(scene_manager: SceneManager, photo_manager: &PhotoManager) -> Self {
        let photos = photo_manager
            .photos
            .iter()
            .map(|photo| Photo {
                path: photo.0.clone(),
            })
            .collect();

        let root_scene = scene_manager.root_scene();

        let app_pages = root_scene
            .edit
            .read()
            .unwrap()
            .state
            .pages_state
            .pages
            .clone();

        let pages: Vec<CanvasPage> = app_pages
            .values()
            .map(|canvas_state| {
                let layers = canvas_state
                    .layers
                    .values()
                    .map(|layer| Layer {
                        content: match layer.content.clone() {
                            AppLayerContent::Photo(canvas_photo) => {
                                LayerContent::Photo(CanvasPhoto {
                                    photo: Photo {
                                        path: canvas_photo.photo.path,
                                    },
                                })
                            }
                            AppLayerContent::Text(canvas_text) => LayerContent::Text(CanvasText {
                                text: canvas_text.text,
                                font_size: canvas_text.font_size,
                                font_id: canvas_text.font_id,
                                color: canvas_text.color,
                            }),
                            AppLayerContent::TemplatePhoto {
                                region,
                                photo,
                                scale_mode,
                            } => LayerContent::TemplatePhoto {
                                region: TemplateRegion {
                                    relative_position: region.relative_position,
                                    relative_size: region.relative_size,
                                    kind: match region.kind {
                                        AppTemplateRegionKind::Image => TemplateRegionKind::Image,
                                        AppTemplateRegionKind::Text {
                                            sample_text,
                                            font_size,
                                        } => TemplateRegionKind::Text {
                                            sample_text,
                                            font_size,
                                        },
                                    },
                                },
                                photo: photo.map(|canvas_photo| CanvasPhoto {
                                    photo: Photo {
                                        path: canvas_photo.photo.path,
                                    },
                                }),
                                scale_mode: match scale_mode {
                                    AppScaleMode::Fit => ScaleMode::Fit,
                                    AppScaleMode::Fill => ScaleMode::Fill,
                                    AppScaleMode::Stretch => ScaleMode::Stretch,
                                },
                            },
                            AppLayerContent::TemplateText { region, text } => {
                                LayerContent::TemplateText {
                                    region: TemplateRegion {
                                        relative_position: region.relative_position,
                                        relative_size: region.relative_size,
                                        kind: match region.kind {
                                            AppTemplateRegionKind::Image => TemplateRegionKind::Image,
                                            AppTemplateRegionKind::Text {
                                                sample_text,
                                                font_size,
                                            } => TemplateRegionKind::Text {
                                                sample_text,
                                                font_size,
                                            },
                                        },
                                    },
                                    text: CanvasText {
                                        text: text.text,
                                        font_size: text.font_size,
                                        font_id: text.font_id,
                                        color: text.color,
                                    },
                                }
                            
                            },
                        },
                        name: layer.name.clone(),
                        visible: layer.visible,
                        locked: layer.locked,
                        selected: layer.selected,
                        id: layer.id,
                        rect: layer.transform_state.rect,
                    })
                    .collect();

                CanvasPage {
                    layers: layers,
                    page: Page {
                        size: canvas_state.page.size(),
                        ppi: canvas_state.page.ppi(),
                        unit: match canvas_state.page.unit() {
                            AppUnit::Pixels => Unit::Pixels,
                            AppUnit::Inches => Unit::Inches,
                            AppUnit::Centimeters => Unit::Centimeters,
                        },
                    },
                }
            })
            .collect();

        Self { photos, pages }
    }

    pub fn load(project: Self, photo_manager: &PhotoManager) -> SceneManager {
       todo!()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasPage {
    pub layers: Vec<Layer>,
    pub page: Page,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    size: Vec2,
    ppi: i32,
    unit: Unit,
}

#[derive(Debug, Clone, PartialEq, Copy, Serialize, Deserialize)]
pub enum Unit {
    Pixels,
    Inches,
    Centimeters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Photo {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub content: LayerContent,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub selected: bool,
    pub id: usize,
    pub rect: Rect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScaleMode {
    Fit,
    Fill,
    Stretch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateRegion {
    pub relative_position: Pos2,
    pub relative_size: Vec2,
    pub kind: TemplateRegionKind,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum TemplateRegionKind {
    Image,
    Text { sample_text: String, font_size: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasPhoto {
    pub photo: Photo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasText {
    pub text: String,
    pub font_size: f32,
    pub font_id: FontId,
    pub color: Color32,
    // layout: Layout TODO
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayerContent {
    Photo(CanvasPhoto),
    Text(CanvasText),
    TemplatePhoto {
        region: TemplateRegion,
        photo: Option<CanvasPhoto>,
        scale_mode: ScaleMode,
    },
    TemplateText {
        region: TemplateRegion,
        text: CanvasText,
    },
}
