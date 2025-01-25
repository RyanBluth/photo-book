use eframe::egui::{self, CursorIcon, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use eframe::emath::Rot2;
use eframe::epaint::{Color32, Mesh, Shape};

use crate::dependencies::{Dependency, Singleton, SingletonFor};
use crate::photo_manager::PhotoManager;
use crate::scene::canvas_scene::{CanvasHistoryKind, CanvasHistoryManager};
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

                let transform_response = TransformableWidget::new(&mut self.crop_state.transform_state).show(
                    ui,
                    self.available_rect,
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

        ui.painter().rect_stroke(
            self.crop_state.transform_state.rect,
            0.0,
            Stroke::new(2.0, Color32::GREEN),
        );

        if self.show_action_bar(ui) {
            return CropResponse::Exit;
        }

        CropResponse::None
    }

    fn show_action_bar(&mut self, ui: &mut Ui) -> bool {
        let bar_rect = Rect::from_min_size(
            Pos2::new(ui.max_rect().center().x - 100.0, ui.max_rect().max.y - 50.0),
            Vec2::new(200.0, 40.0),
        );

        ui.allocate_ui_at_rect(bar_rect, |ui| {
            ui.horizontal_centered(|ui| {
                if ui.button("Apply").clicked() {
                    // Update the target layer's crop rect
                    if let Some(layer) = self.state.layers.get_mut(&self.crop_state.target_layer) {
                        if let LayerContent::Photo(photo) = &mut layer.content {
                            // Calculate a normalized crop rect by intersecting the crop rect with the photo rect and then normalizing it
                            let intersection = self.crop_state.photo_rect.intersect(self.crop_state.transform_state.rect);
                            let normalized_intersection = Rect::from_min_size(
                                Pos2::new(
                                    (intersection.min - self.crop_state.photo_rect.min).x / self.crop_state.photo_rect.size().x,
                                    (intersection.min - self.crop_state.photo_rect.min).y / self.crop_state.photo_rect.size().y
                                ),
                                Vec2::new(
                                    intersection.size().x / self.crop_state.photo_rect.size().x,
                                    intersection.size().y / self.crop_state.photo_rect.size().y
                                ),
                            );
                            if let Some(layer) = self.state.layers.get_mut(&self.crop_state.target_layer) {
                                if let LayerContent::Photo(photo) = &mut layer.content {
                                    photo.crop = normalized_intersection;
                                }
                            }
                        }
                    }
                    return true;
                }
                if ui.button("Cancel").clicked() {
                    if let Some(layer) = self.state.layers.get_mut(&self.crop_state.target_layer) {
                        if let LayerContent::Photo(photo) = &mut layer.content {
                            photo.crop = self.crop_state.original_crop;
                        }
                    }
                    return true;
                }

                false
            })
            .inner
        })
        .inner
    }
}
