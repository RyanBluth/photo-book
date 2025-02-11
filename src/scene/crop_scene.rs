use crate::{
    id::{LayerId, PageId},
    photo::Photo,
    utils::{IdExt, RectExt},
    widget::{
        canvas::{CanvasPhoto, CanvasState},
        canvas_state::CropState,
        crop::{Crop, CropResponse},
        transformable::{ResizeMode, TransformHandleMode, TransformableState},
    },
};
use egui::{Id, Pos2, Rect, Vec2};

use super::{Scene, ScenePopResponse, SceneResponse};

pub enum CropSceneResponse {
    Apply {
        layer_id: LayerId,
        page_id: PageId,
        crop: Rect,
    },
}

pub struct CropScene {
    target_layer: LayerId,
    target_page: PageId,
    transform_state: TransformableState,
    photo_rect: Rect,
    photo: Photo,
}

impl CropScene {
    pub fn new(
        target_layer: LayerId,
        target_page: PageId,
        rect: Rect,
        photo: Photo,
        initial_crop: Rect,
    ) -> Self {
        let padded_available_rect: Rect =
            rect.shrink2(Vec2::new(rect.width() * 0.1, rect.height() * 0.1));

        let mut photo_rect = padded_available_rect
            .with_aspect_ratio(photo.metadata.width() as f32 / photo.metadata.height() as f32);

        photo_rect = photo_rect.fit_and_center_within(padded_available_rect);

        let crop_origin = Pos2::new(
            photo_rect.width() * initial_crop.left_top().x,
            photo_rect.height() * initial_crop.left_top().y,
        );

        let mut scaled_crop_rect: Rect = Rect::from_min_max(
            crop_origin,
            Pos2::new(
                crop_origin.x + photo_rect.width() * initial_crop.width(),
                crop_origin.y + photo_rect.height() * initial_crop.height(),
            ),
        );

        let rotation = photo.metadata.rotation().radians();

        scaled_crop_rect = scaled_crop_rect
            .to_world_space(photo_rect)
            .rotate_bb_around_point(rotation, photo_rect.center());

        photo_rect = photo_rect.rotate_bb_around_center(rotation);

        scaled_crop_rect = scaled_crop_rect.to_local_space(photo_rect);

        let transform_state = TransformableState {
            rect: scaled_crop_rect,
            rotation: 0.0,
            handle_mode: TransformHandleMode::Resize(ResizeMode::Free),
            active_handle: None,
            is_moving: false,
            last_frame_rotation: 0.0,
            change_in_rotation: None,
            id: Id::random(),
        };

        Self {
            target_layer,
            target_page,
            transform_state,
            photo_rect,
            photo,
        }
    }
}

impl Scene for CropScene {
    fn ui(&mut self, ui: &mut egui::Ui) -> SceneResponse {
        let mut crop_state = CropState {
            target_layer: self.target_layer,
            transform_state: self.transform_state.clone(),
            photo_rect: self.photo_rect,
            photo: self.photo.clone(),
        };

        match Crop::new(ui.available_rect_before_wrap(), &mut crop_state).show(ui) {
            CropResponse::Apply(crop) => {
                SceneResponse::Pop(ScenePopResponse::Crop(CropSceneResponse::Apply {
                    layer_id: self.target_layer,
                    page_id: self.target_page,
                    crop,
                }))
            }
            CropResponse::Exit => SceneResponse::Pop(ScenePopResponse::None),
            CropResponse::None => {
                self.transform_state = crop_state.transform_state;
                SceneResponse::None
            }
        }
    }
}
