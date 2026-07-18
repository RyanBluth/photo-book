use eframe::egui::{self, Pos2, Rect, Ui, Vec2};
use egui::UiBuilder;

use crate::model::photo_adjustments::PhotoAdjustments;
use crate::photo::Photo;
use crate::photo_renderer::{PhotoRenderOptions, PhotoRenderer};
use crate::theme::color;
use crate::utils::RectExt;
use crate::widget::action_bar::{ActionBar, ActionBarResponse, ActionItem, ActionItemKind};
use crate::widget::auto_center::AutoCenter;
use crate::widget::canvas_state::CropState;
use crate::widget::transformable::TransformableWidget;

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

        self.crop_state
            .photo_rect
            .set_center(ui.max_rect().center());

        let render_key = format!("crop:{}:base", self.crop_state.target_layer);
        let _ = PhotoRenderer::paint(
            ui,
            self.photo,
            self.adjustments,
            self.crop_state.photo_rect,
            PhotoRenderOptions::default()
                .with_clip_rect(ui.max_rect())
                .with_render_key(&render_key),
        );

        ui.painter().rect_filled(ui.max_rect(), 0.0, color::OVERLAY);

        let crop_clip = self
            .crop_state
            .transform_state
            .rect
            .to_world_space(self.crop_state.photo_rect);
        let selection_render_key = format!("crop:{}:selection", self.crop_state.target_layer);
        let _ = PhotoRenderer::paint(
            ui,
            self.photo,
            self.adjustments,
            self.crop_state.photo_rect,
            PhotoRenderOptions::default()
                .with_clip_rect(crop_clip)
                .with_render_key(&selection_render_key),
        );

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
