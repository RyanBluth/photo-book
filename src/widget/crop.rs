use std::thread::current;

use eframe::egui::{self, CursorIcon, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use eframe::emath::Rot2;
use eframe::epaint::{Color32, Mesh, Shape};
use egui::UiBuilder;

use crate::dependencies::{Dependency, Singleton, SingletonFor};
use crate::photo_manager::PhotoManager;
use crate::scene::canvas_scene::{CanvasHistoryKind, CanvasHistoryManager};
use crate::utils::RectExt;
use crate::widget::action_bar::{ActionBar, ActionBarResponse, ActionItem, ActionItemKind};
use crate::widget::auto_center::AutoCenter;
use crate::widget::canvas::CanvasState;
use crate::widget::canvas_info::layers::LayerContent;
use crate::widget::canvas_state::{CanvasInteractionMode, CropState};
use crate::widget::transformable::{ResizeMode, TransformHandleMode, TransformableWidget};

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum CropResponse {
    Exit,
    None,
}

pub struct Crop<'a> {
    pub state: &'a mut CanvasState,
    pub available_rect: Rect,
    pub history_manager: &'a mut CanvasHistoryManager,
    pub crop_state: &'a mut CropState,
}

impl<'a> Crop<'a> {
    pub fn new(
        state: &'a mut CanvasState,
        available_rect: Rect,
        history_manager: &'a mut CanvasHistoryManager,
        crop_state: &'a mut CropState,
    ) -> Self {
        Self {
            state,
            available_rect,
            history_manager,
            crop_state,
        }
    }

    pub fn show(&mut self, ui: &mut Ui) -> CropResponse {
        ui.painter()
            .rect_filled(self.available_rect, 0.0, Color32::BLACK);

        if let Some(layer) = self.state.layers.get(&self.crop_state.target_layer) {
            if let LayerContent::Photo(photo) = &layer.content {
                let texture = Dependency::<PhotoManager>::get()
                    .with_lock_mut(|photo_manager| {
                        photo_manager.texture_for_photo_with_thumbail_backup(&photo.photo, ui.ctx())
                    })
                    .unwrap()
                    .unwrap(); // TODO: Don't unwrap

                let painter: &egui::Painter = ui.painter();
                let mut mesh: Mesh = Mesh::with_texture(texture.id);

                let mesh_rect =
                    if photo.photo.metadata.width() != photo.photo.metadata.rotated_width() {
                        Rect::from_center_size(
                            self.crop_state.photo_rect.center(),
                            Vec2::new(
                                self.crop_state.photo_rect.height(),
                                self.crop_state.photo_rect.width(),
                            ),
                        )
                    } else {
                        self.crop_state.photo_rect
                    };

                mesh.add_rect_with_uv(
                    mesh_rect,
                    Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0)),
                    Color32::WHITE,
                );

                let mesh_center: Pos2 = self.crop_state.photo_rect.min
                    + Vec2::splat(0.5) * self.crop_state.photo_rect.size();

                mesh.rotate(
                    Rot2::from_angle(photo.photo.metadata.rotation().radians()),
                    mesh_center,
                );

                painter.add(Shape::mesh(mesh));

                let transform_response =
                    TransformableWidget::new(&mut self.crop_state.transform_state).show(
                        ui,
                        mesh_rect,
                        1.0,
                        true,
                        |_ui: &mut Ui, _transformed_rect: Rect, _transformable_state| {},
                    );

                if transform_response.ended_moving
                    || transform_response.ended_resizing
                    || transform_response.ended_rotating
                {
                    self.history_manager
                        .save_history(CanvasHistoryKind::Transform, self.state);
                }
            }
        }

        if self.show_action_bar(ui) {
            return CropResponse::Exit;
        }

        CropResponse::None
    }

    fn show_action_bar(&mut self, ui: &mut Ui) -> bool {
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
                kind: ActionItemKind::Text("Apply".to_string()),
                action: "apply",
            },
            ActionItem {
                kind: ActionItemKind::Text("Cancel".to_string()),
                action: "cancel",
            },
        ];

        match ui
            .allocate_new_ui(UiBuilder::new().max_rect(bar_rect), |ui| {
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
                "apply" => {
                    // Update the target layer's crop rect
                    if let Some(layer) = self.state.layers.get_mut(&self.crop_state.target_layer) {
                        if let LayerContent::Photo(photo) = &mut layer.content {

                            let world_transform_rect = self
                                .crop_state
                                .transform_state
                                .rect
                                .to_world_space(self.crop_state.photo_rect);

                            let intersection =
                                world_transform_rect.intersect(self.crop_state.photo_rect);

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
                            if let Some(layer) =
                                self.state.layers.get_mut(&self.crop_state.target_layer)
                            {
                                if let LayerContent::Photo(photo) = &mut layer.content {                                   
                                    photo.crop = normalized_intersection;

                                    let crop_aspect_ratio = normalized_intersection.width() / normalized_intersection.height();
                                    let mut transform_rect = layer.transform_state.rect;
                                    let rect_center = transform_rect.center();

                                    let old_w = transform_rect.width();
                                    let old_h = transform_rect.height();
                                    let old_ar = old_w / old_h;

                                    if old_ar < crop_aspect_ratio {
                                        // Keep width, shrink height
                                        let new_h = old_w / crop_aspect_ratio;
                                        transform_rect = Rect::from_center_size(rect_center, Vec2::new(old_w, new_h));
                                    } else {
                                        // Keep height, shrink width
                                        let new_w = old_h * crop_aspect_ratio;
                                        transform_rect = Rect::from_center_size(rect_center, Vec2::new(new_w, old_h));
                                    }

                                    layer.transform_state.rect = transform_rect;
                                }
                            }
                        }
                    }
                    true
                }
                "cancel" => true,
                _ => false,
            },
            _ => false,
        }
    }
}
