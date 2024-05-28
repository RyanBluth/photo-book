use std::{
    any::Any,
    borrow::Borrow,
    sync::{Arc, RwLock},
};

use egui::{Ui, Vec2};
use sqlx::Either;

use super::{
    canvas_scene::CanvasScene,
    organize_scene::GalleryScene,
    Scene, SceneResponse,
    SceneTransition::{self, Viewer},
};

pub struct OrganizeEditScene {
    organize: Arc<RwLock<GalleryScene>>,
    edit: Arc<RwLock<CanvasScene>>,
    current: Either<Arc<RwLock<GalleryScene>>, Arc<RwLock<CanvasScene>>>,
}

impl OrganizeEditScene {
    pub fn new(organize: GalleryScene, edit: CanvasScene) -> Self {
        let organize_scene = Arc::new(RwLock::new(organize));
        Self {
            organize: organize_scene.clone(),
            edit: Arc::new(RwLock::new(edit)),
            current: Either::Left(organize_scene.clone()),
        }
    }

    pub fn show_organize(&mut self) {
        self.current = Either::Left(self.organize.clone());
        // TODO: This is a bit of a hack to keep the gallery state in sync between the two scenes
        // Introduce some sort of shared state between the two scenes
        self.organize.write().unwrap().state.image_gallery_state = self
            .edit
            .read()
            .unwrap()
            .state
            .gallery_state
            .clone();
    }

    pub fn show_edit(&mut self) {
        self.current = Either::Right(self.edit.clone());
        // TODO: This is a bit of a hack to keep the gallery state in sync between the two scenes
        // Introduce some sort of shared state between the two scenes
        self.edit.write().unwrap().state.gallery_state = self
            .organize
            .read()
            .unwrap()
            .state
            .image_gallery_state
            .clone();
    }
}

impl Scene for OrganizeEditScene {
    fn ui(&mut self, ui: &mut Ui) -> SceneResponse {
        ui.vertical(|ui| {
            ui.allocate_ui(Vec2::new(ui.available_width(), 50.0), |ui| {
                let top_nav_button_width: f32 = ui.memory_mut(|memory| {
                    memory
                        .data
                        .get_persisted("top_nav_button_width".into())
                        .unwrap_or_default()
                });

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    ui.add_space(ui.available_width() / 2.0 - top_nav_button_width / 2.0);

                    let nav_buttons_response = ui.horizontal(|ui| {
                        if ui.button("Organize").clicked() {
                            self.show_organize();
                        }
                        if ui.button("Edit").clicked() {
                            self.show_edit();
                        }
                    });

                    ui.memory_mut(|memory| {
                        memory.data.insert_persisted(
                            "top_nav_button_width".into(),
                            nav_buttons_response.response.rect.width(),
                        );
                    });
                });
            });

            ui.add_space(10.0);

            let scene_response = ui
                .allocate_ui(
                    Vec2::new(ui.available_width(), ui.available_height() - 50.0),
                    |ui| match &self.current {
                        Either::Left(organize) => {
                            let mut organize = organize.write().unwrap();
                            organize.ui(ui)
                        }
                        Either::Right(edit) => {
                            let mut edit = edit.write().unwrap();
                            edit.ui(ui)
                        }
                    },
                )
                .inner;

            // Act as the navigator for certain scene transitions
            // TODO: Is there a more elegant way to do this?
            match scene_response {
                SceneResponse::Push(transition) => match transition {
                    SceneTransition::Gallery(scene) => {
                        *self.organize.write().unwrap() = scene;
                        self.show_organize();
                        SceneResponse::None
                    }
                    SceneTransition::Canvas(mut scene) => {
                        scene.state.gallery_state = self
                            .organize
                            .read()
                            .unwrap()
                            .state
                            .image_gallery_state
                            .clone();
                        *self.edit.write().unwrap() = scene;
                        self.show_edit();
                        SceneResponse::None
                    }
                    _ => SceneResponse::Push(transition),
                },
                _ => scene_response,
            }
        })
        .inner
    }
}
