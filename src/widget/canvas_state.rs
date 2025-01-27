use crate::id::LayerId;
use eframe::egui::{Id, Rect};
use crate::widget::transformable::TransformableState;

#[derive(Debug, Clone, PartialEq)]
pub enum CanvasInteractionMode {
    Normal,
    Crop(CropState),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CropState {
    pub target_layer: LayerId,
    pub transform_state: TransformableState,
    pub photo_rect: Rect,
}
