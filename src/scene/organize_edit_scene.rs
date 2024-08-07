use std::{
    any::Any,
    borrow::Borrow,
    sync::{Arc, RwLock},
};

use egui::{menu, Color32, Pos2, Rect, Ui, Vec2};
use log::{error, info};
use sqlx::Either;

use crate::{
    auto_persisting::AutoPersisting,
    config::{Config, ConfigModification},
    dependencies::{Dependency, Singleton, SingletonFor},
    photo_manager::PhotoManager,
    project::v1::Project,
};

use super::{
    canvas_scene::CanvasScene,
    organize_scene::{GalleryScene, GallerySceneState},
    Scene, SceneResponse,
    SceneTransition::{self, Viewer},
};

pub struct OrganizeEditScene {
    pub organize: Arc<RwLock<GalleryScene>>,
    pub edit: Arc<RwLock<CanvasScene>>,
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
        self.organize.write().unwrap().state.image_gallery_state =
            self.edit.read().unwrap().state.gallery_state.clone();
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
        ui.painter().rect_filled(
            Rect::from_min_max(Pos2::ZERO, Pos2::new(ui.max_rect().width() + 100.0, 50.0)),
            0.0,
            Color32::from_gray(40),
        );

        ui.vertical(|ui| {
            ui.allocate_ui(Vec2::new(ui.available_width(), 50.0), |ui| {
                let top_nav_button_width: f32 = ui.memory_mut(|memory: &mut egui::Memory| {
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

            ui.add_space(12.0);

            menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open").clicked() {
                        let open_path = native_dialog::FileDialog::new()
                            .add_filter("Images", &["rpb"])
                            .show_open_single_file();

                        match open_path {
                            Ok(Some(open_path)) => {
                                let photo_manager: Singleton<PhotoManager> = Dependency::get();

                                photo_manager.with_lock_mut(|photo_manager| {
                                    match Project::load(&open_path, photo_manager) {
                                        Ok(scene) => {
                                            let config: Singleton<AutoPersisting<Config>> =
                                                Dependency::get();
                                            config.with_lock_mut(|config| {
                                                config.modify(
                                                    ConfigModification::AddRecentProject(
                                                        open_path.clone(),
                                                    ),
                                                );
                                            });

                                            *self = scene;
                                            self.show_organize();
                                        }
                                        Err(err) => {
                                            error!("Error loading project: {:?}", err);
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                error!("Error opening open file dialog: {:?}", e);
                            }
                            Ok(None) => {
                                info!("No open path selected");
                            }
                        }
                    }

                    ui.menu_button("Open Recent", |ui| {
                        let config: Singleton<AutoPersisting<Config>> = Dependency::get();
                        let recents = config.with_lock_mut(|config| {
                            config.read().unwrap().recent_projects().to_vec()
                        });

                        if recents.is_empty() {
                            ui.label("No recent projects");
                        } else {
                            let photo_manager: Singleton<PhotoManager> = Dependency::get();
                            photo_manager.with_lock_mut(|photo_manager| {
                                for recent in &recents {
                                    if ui.button(&recent.display().to_string()).clicked() {
                                        match Project::load(&recent.into(), photo_manager) {
                                            Ok(scene) => {
                                                config.with_lock_mut(|config| {
                                                    config.modify(
                                                        ConfigModification::AddRecentProject(
                                                            recent.into(),
                                                        ),
                                                    );
                                                });

                                                *self = scene;
                                                self.show_organize();
                                            }
                                            Err(err) => {
                                                error!("Error loading project: {:?}", err);
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    });

                    if ui.button("Save").clicked() {
                        let save_path = native_dialog::FileDialog::new()
                            .add_filter("Images", &["rpb"])
                            .show_save_single_file();

                        match save_path {
                            Ok(Some(save_path)) => {
                                let photo_manager: Singleton<PhotoManager> = Dependency::get();

                                photo_manager.with_lock(|photo_manager| {
                                    if let Err(err) =
                                        Project::save(&save_path, &self, photo_manager)
                                    {
                                        error!("Error saving project: {:?}", err);
                                    }
                                });
                            }
                            Err(e) => {
                                error!("Error opening save file dialog: {:?}", e);
                            }
                            Ok(None) => {
                                info!("No save path selected");
                            }
                        }
                    }

                    if ui.button("Import").clicked() {
                        let import_dir = native_dialog::FileDialog::new()
                            .add_filter("Images", &["png", "jpg", "jpeg"])
                            .show_open_single_dir();

                        match import_dir {
                            Ok(Some(import_dir)) => {
                                info!("Imported {:?}", import_dir);
                                let _ = PhotoManager::load_directory(import_dir.clone());
                            }
                            Err(e) => {
                                error!("Error opening import file dialog: {:?}", e);
                            }
                            Ok(None) => {
                                info!("No import directory selected");
                            }
                        }
                    }

                    if ui.button("Export").clicked() {}
                });
            });

            let scene_response = ui
                .allocate_ui(
                    Vec2::new(ui.available_width(), ui.available_height()),
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
