use crate::{id::LayerId, photo::Photo};
use crate::widget::transformable::TransformableState;
use eframe::egui::{Id, Rect};

#[derive(Debug, Clone, PartialEq)]
pub struct CropState {
    pub target_layer: LayerId,
    pub transform_state: TransformableState,
    pub photo_rect: Rect,
    pub photo: Photo,
}
