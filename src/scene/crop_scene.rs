use crate::{
    id::{LayerId, PageId},
    photo::Photo,
    utils::{IdExt, RectExt},
    widget::{
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

        let photo_rect = padded_available_rect
            .with_aspect_ratio(
                photo.metadata.rotated_width() as f32 / photo.metadata.rotated_height() as f32,
            )
            .fit_and_center_within(padded_available_rect);

        // Calculate crop rect based on the adjusted photo_rect
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
        let available_rect = ui.max_rect();
        let padded_available_rect: Rect = available_rect.shrink2(Vec2::new(
            available_rect.width() * 0.1,
            available_rect.height() * 0.1,
        ));

        let adjusted_photo_rect = self.photo_rect.fit_and_center_within(padded_available_rect);

        let scale_factor: f32 = (adjusted_photo_rect.width() / self.photo_rect.width())
            .min(adjusted_photo_rect.height() / self.photo_rect.height());

        let transform_position = self.transform_state.rect.left_top();
        let transform_size = self.transform_state.rect.size();

        self.transform_state
            .rect
            .set_left(transform_position.x * scale_factor);
        self.transform_state
            .rect
            .set_top(transform_position.y * scale_factor);

        self.transform_state
            .rect
            .set_width(transform_size.x * scale_factor);
        self.transform_state
            .rect
            .set_height(transform_size.y * scale_factor);

        self.photo_rect = adjusted_photo_rect;

        let mut crop_state = CropState {
            target_layer: self.target_layer,
            transform_state: self.transform_state.clone(),
            photo_rect: self.photo_rect,
            photo: self.photo.clone(),
        };

        match Crop::new(&mut crop_state).show(ui) {
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
