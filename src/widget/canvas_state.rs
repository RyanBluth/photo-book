use crate::id::LayerId;
use crate::widget::transformable::TransformableState;
use eframe::egui::Rect;

#[derive(Debug, Clone, PartialEq)]
pub struct CropState {
    pub target_layer: LayerId,
    pub transform_state: TransformableState,
    pub photo_rect: Rect,
}
