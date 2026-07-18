use eframe::egui::{self, Pos2, Rect, Ui, Vec2};
use eframe::epaint::{Mesh, Shape};
use egui::UiBuilder;

use crate::gpu_photo_adjustment::{
    GpuPhotoAdjustmentRenderer, GpuPhotoPaintRequest, GpuPhotoSource,
};
use crate::model::photo_adjustments::PhotoAdjustments;
use crate::photo::Photo;
use crate::photo_manager::PhotoManager;
use crate::theme::color;
use crate::utils::RectExt;
use crate::widget::action_bar::{ActionBar, ActionBarResponse, ActionItem, ActionItemKind};
use crate::widget::auto_center::AutoCenter;
use crate::widget::canvas_state::CropState;
use crate::widget::transformable::TransformableWidget;
use crate::{dep, dep_mut};

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum CropResponse {
    Apply(Rect),
    Exit,
    None,
}

#[derive(Debug, Clone, PartialEq, Copy)]
enum CropActionBarResponse {
    Apply,
    Cancel,
}

pub struct Crop<'a> {
    pub crop_state: &'a mut CropState,
    photo: &'a Photo,
    adjustments: &'a PhotoAdjustments,
}

impl<'a> Crop<'a> {
    pub fn new(
        crop_state: &'a mut CropState,
        photo: &'a Photo,
        adjustments: &'a PhotoAdjustments,
    ) -> Self {
        Self {
            crop_state,
            photo,
            adjustments,
        }
    }

    pub fn show(&mut self, ui: &mut Ui) -> CropResponse {
        ui.painter().rect_filled(ui.max_rect(), 0.0, color::BLACK);

        let texture = dep_mut!(PhotoManager, |photo_manager| {
            if self.adjustments.is_identity() {
                photo_manager
                    .unadjusted_texture_for_photo_with_thumbnail_fallback(self.photo, ui.ctx())
            } else {
                photo_manager.unadjusted_gpu_placeholder_texture_for(self.photo, ui.ctx())
            }
        })
        .ok()
        .flatten();

        self.crop_state
            .photo_rect
            .set_center(ui.max_rect().center());

        let source_uv = Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0));
        let render_key = format!("crop:{}:base", self.crop_state.target_layer);
        let gpu_result = (!self.adjustments.is_identity()).then(|| {
            dep!(GpuPhotoAdjustmentRenderer, |renderer| {
                renderer.paint(
                    ui,
                    GpuPhotoPaintRequest {
                        photo: self.photo,
                        source: GpuPhotoSource::FullResolution,
                        clip_rect: ui.max_rect(),
                        rect: self.crop_state.photo_rect,
                        source_uv,
                        rotation_radians: 0.0,
                        adjustments: self.adjustments,
                        render_key: Some(&render_key),
                    },
                )
            })
        });
        let adjusted_texture = if gpu_result.is_some_and(|result| result.requires_cpu_fallback()) {
            dep_mut!(PhotoManager, |photo_manager| {
                photo_manager
                    .texture_for_photo_with_thumbnail_fallback(
                        self.photo,
                        self.adjustments,
                        ui.ctx(),
                    )
                    .ok()
                    .flatten()
            })
        } else {
            None
        };

        if let Some(adjusted_texture) = adjusted_texture {
            Self::paint_texture(
                ui.painter(),
                adjusted_texture.id,
                self.crop_state.photo_rect,
                source_uv,
            );
        } else if !gpu_result.is_some_and(|result| result.is_ready())
            && let Some(texture) = texture
        {
            Self::paint_texture(
                ui.painter(),
                texture.id,
                self.crop_state.photo_rect,
                source_uv,
            );
        }

        ui.painter().rect_filled(ui.max_rect(), 0.0, color::OVERLAY);

        let crop_clip = self
            .crop_state
            .transform_state
            .rect
            .to_world_space(self.crop_state.photo_rect);
        let clipped_painter = ui.painter().with_clip_rect(crop_clip);
        let selection_gpu_result = (!self.adjustments.is_identity()).then(|| {
            let selection_render_key = format!("crop:{}:selection", self.crop_state.target_layer);
            dep!(GpuPhotoAdjustmentRenderer, |renderer| {
                renderer.paint(
                    ui,
                    GpuPhotoPaintRequest {
                        photo: self.photo,
                        source: GpuPhotoSource::FullResolution,
                        clip_rect: crop_clip,
                        rect: self.crop_state.photo_rect,
                        source_uv,
                        rotation_radians: 0.0,
                        adjustments: self.adjustments,
                        render_key: Some(&selection_render_key),
                    },
                )
            })
        });

        if let Some(adjusted_texture) = adjusted_texture {
            Self::paint_texture(
                &clipped_painter,
                adjusted_texture.id,
                self.crop_state.photo_rect,
                source_uv,
            );
        } else if !selection_gpu_result.is_some_and(|result| result.is_ready())
            && let Some(texture) = texture
        {
            Self::paint_texture(
                &clipped_painter,
                texture.id,
                self.crop_state.photo_rect,
                source_uv,
            );
        }

        let _ = TransformableWidget::new(&mut self.crop_state.transform_state).show(
            ui,
            self.crop_state.photo_rect,
            1.0,
            true,
            false,
            |_ui: &mut Ui, _transformed_rect: Rect, _transformable_state| {},
        );

        self.show_action_bar(ui)
    }

    fn paint_texture(painter: &egui::Painter, texture_id: egui::TextureId, rect: Rect, uv: Rect) {
        let mut mesh = Mesh::with_texture(texture_id);
        mesh.add_rect_with_uv(rect, uv, color::WHITE);
        painter.add(Shape::mesh(mesh));
    }

    fn show_action_bar(&mut self, ui: &mut Ui) -> CropResponse {
        let bar_height = 40.0;
        let bar_margin_bottom = 40.0;

        let bar_rect = Rect::from_min_size(
            Pos2::new(
                ui.max_rect().left(),
                ui.max_rect().max.y - bar_margin_bottom - bar_height / 2.0,
            ),
            Vec2::new(ui.max_rect().width(), bar_height),
        );

        let actions = vec![
            ActionItem {
                kind: ActionItemKind::Text("Cancel".to_string()),
                action: CropActionBarResponse::Cancel,
            },
            ActionItem {
                kind: ActionItemKind::Text("Apply".to_string()),
                action: CropActionBarResponse::Apply,
            },
        ];

        match ui
            .scope_builder(UiBuilder::new().max_rect(bar_rect), |ui| {
                AutoCenter::new("crop_action_bar")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| ActionBar::with_items(actions).show(ui))
                            .inner
                    })
                    .inner
            })
            .inner
        {
            ActionBarResponse::Clicked(action) => match action {
                CropActionBarResponse::Apply => {
                    let world_transform_rect = self
                        .crop_state
                        .transform_state
                        .rect
                        .to_world_space(self.crop_state.photo_rect);

                    let intersection = world_transform_rect.intersect(self.crop_state.photo_rect);

                    let normalized_intersection = Rect::from_min_size(
                        Pos2::new(
                            (intersection.min - self.crop_state.photo_rect.min).x
                                / self.crop_state.photo_rect.size().x,
                            (intersection.min - self.crop_state.photo_rect.min).y
                                / self.crop_state.photo_rect.size().y,
                        ),
                        Vec2::new(
                            intersection.size().x / self.crop_state.photo_rect.size().x,
                            intersection.size().y / self.crop_state.photo_rect.size().y,
                        ),
                    );
                    CropResponse::Apply(normalized_intersection)
                }
                CropActionBarResponse::Cancel => CropResponse::Exit,
            },
            _ => CropResponse::None,
        }
    }
}
