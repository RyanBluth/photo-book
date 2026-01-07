use std::sync::{Arc, RwLock};

use egui::{Color32, CursorIcon, Pos2, Rect, RichText, Sense, Ui, Vec2};
use log::{error, info};

use crate::{
    auto_persisting::AutoPersisting,
    config::Config,
    cursor_manager::CursorManager,
    debug::DebugSettings,
    dependencies::{Dependency, Singleton, SingletonFor},
    export::Exporter,
    modal::{
        ModalActionResponse,
        basic::BasicModal,
        manager::{ModalManager, TypedModalId},
        page_settings::PageSettingsModal,
    },
    model::photo_grouping::PhotoGrouping,
    photo_manager::PhotoManager,
    project_settings::ProjectSettingsManager,
    session::{Session, SessionError},
    utils::{Either, Toggle},
};

use super::{
    Scene, ScenePopResponse, SceneResponse,
    SceneTransition::{self},
    canvas_scene::CanvasScene,
    organize_scene::GalleryScene,
};

#[derive(Debug, Clone)]
pub struct OrganizeEditScene {
    pub organize: Arc<RwLock<GalleryScene>>,
    pub edit: Option<Arc<RwLock<CanvasScene>>>,
    current: Either<Arc<RwLock<GalleryScene>>, Arc<RwLock<CanvasScene>>>,
    page_settings_modal_id: Option<TypedModalId<PageSettingsModal>>,
}

impl OrganizeEditScene {
    pub fn new(organize: GalleryScene, edit: Option<CanvasScene>) -> Self {
        let organize_scene: Arc<RwLock<GalleryScene>> = Arc::new(RwLock::new(organize));
        let edit = edit.map(|edit| Arc::new(RwLock::new(edit)));
        Self {
            organize: organize_scene.clone(),
            edit: edit,
            current: Either::Left(organize_scene.clone()),
            page_settings_modal_id: None,
        }
    }

    pub fn show_organize(&mut self) {
        self.current = Either::Left(self.organize.clone());
        // TODO: This is a bit of a hack to keep the gallery state in sync between the two scenes
        // Introduce some sort of shared state between the two scenes
        if let Some(edit) = &self.edit {
            self.organize.write().unwrap().state.image_gallery_state =
                edit.read().unwrap().state.gallery_state.clone();
        }
    }

    pub fn show_edit(&mut self) {
        let project_settings_manager: Singleton<ProjectSettingsManager> = Dependency::get();

        if project_settings_manager.with_lock(|project_settings_manager| {
            project_settings_manager
                .project_settings
                .default_page
                .is_none()
        }) {
            self.page_settings_modal_id = Some(ModalManager::push(PageSettingsModal::new()));
            return;
        }

        if let Some(edit) = &self.edit {
            self.current = Either::Right(edit.clone());
        }

        if let Some(edit) = &self.edit {
            // TODO: This is a bit of a hack to keep the gallery state in sync between the two scenes
            // Introduce some sort of shared state between the two scene
            edit.write().unwrap().state.gallery_state = self
                .organize
                .read()
                .unwrap()
                .state
                .image_gallery_state
                .clone();
        }
    }

    fn mode_selector(&mut self, ui: &mut Ui) {
        ui.style_mut().interaction.selectable_labels = false;

        let mut organize_text = RichText::new("Organize");
        let mut edit_text = RichText::new("Edit");

        if self.current.is_left() {
            organize_text = organize_text.strong();
        } else {
            edit_text = edit_text.strong();
        }

        let organize_heading = ui.heading(organize_text).interact(Sense::click());
        let edit_heading = ui.heading(edit_text).interact(Sense::click());

        if organize_heading.hovered()
            || edit_heading.hovered()
            || organize_heading.is_pointer_button_down_on()
            || edit_heading.is_pointer_button_down_on()
        {
            let cursor_manager: Singleton<CursorManager> = Dependency::get();
            cursor_manager.with_lock_mut(|cursor_manager| {
                cursor_manager.set_cursor(CursorIcon::PointingHand);
            });
        }

        if organize_heading.clicked() {
            self.show_organize();
        }

        if edit_heading.clicked() {
            self.show_edit();
        }
        if let Some(id) = &self.page_settings_modal_id {
            let modal_manager: Singleton<ModalManager> = Dependency::get();

            let exists = modal_manager.with_lock(|modal_manager| modal_manager.exists(id));

            let modal_response =
                modal_manager.with_lock(|modal_manager| modal_manager.response_for(id));
            match modal_response {
                Ok(Some(response)) => match response {
                    ModalActionResponse::Confirm => {
                        if self.edit.is_none() {
                            self.edit = Some(Arc::new(RwLock::new(CanvasScene::new())));
                        }
                        self.show_edit();
                    }
                    _ => {}
                },
                _ => {}
            }

            if !exists {
                self.page_settings_modal_id = None;
            }
        }
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
                        self.mode_selector(ui);
                    });

                    ui.memory_mut(|memory| {
                        memory.data.insert_persisted(
                            "top_nav_button_width".into(),
                            nav_buttons_response.response.rect.width(),
                        );
                    });
                });
            });

            ui.add_space(-20.0);

            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Project").clicked() {
                        Dependency::<Session>::get().with_lock_mut(|session| {
                            match session.new_project() {
                                Ok(scene) => {
                                    *self = scene;
                                    self.show_organize();
                                }
                                Err(SessionError::WaitingForUserInput) => {}
                                Err(e) => {
                                    error!("Error creating new project: {:?}", e);
                                }
                            }
                        })
                    }

                    if ui.button("Open").clicked() {
                        Dependency::<Session>::get().with_lock_mut(|session| {
                            match session.load_project(None) {
                                Ok(scene) => {
                                    *self = scene;
                                    self.show_organize();
                                }
                                Err(SessionError::WaitingForUserInput) => {}
                                Err(err) => {
                                    error!("Error loading project: {:?}", err);

                                    ModalManager::push(BasicModal::new(
                                        "Error",
                                        format!("Error loading project: {:?}", err),
                                        "OK",
                                    ));
                                }
                            }
                        })
                    }

                    ui.menu_button("Open Recent", |ui| {
                        let config: Singleton<AutoPersisting<Config>> = Dependency::get();
                        let recents = config.with_lock_mut(|config| {
                            config.read().unwrap().recent_projects().to_vec()
                        });

                        if recents.is_empty() {
                            ui.label("No recent projects");
                        } else {
                            for recent in &recents {
                                if ui.button(recent.display().to_string()).clicked() {
                                    match Dependency::<Session>::get().with_lock_mut(|session| {
                                        session.load_project(Some(recent.clone()))
                                    }) {
                                        Ok(scene) => {
                                            *self = scene;
                                            self.show_organize();
                                        }
                                        Err(SessionError::WaitingForUserInput) => {}
                                        Err(err) => {
                                            error!("Error loading project: {:?}", err);

                                            ModalManager::push(BasicModal::new(
                                                "Error",
                                                format!("Error loading project: {:?}", err),
                                                "OK",
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    });

                    if ui.button("Save").clicked() {
                        if let Err(err) = Dependency::<Session>::get()
                            .with_lock_mut(|session| session.save_project(&self))
                        {
                            error!("Error saving project: {:?}", err);
                        }
                    }

                    if ui.button("Import").clicked() {
                        let import_dir = native_dialog::DialogBuilder::file()
                            .add_filter("Images", &["png", "jpg", "jpeg"])
                            .open_single_dir()
                            .show();

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

                    if ui.button("Export").clicked() {
                        let export_path = native_dialog::DialogBuilder::file()
                            .set_filename("export.pdf")
                            .save_single_file()
                            .show();

                        match export_path {
                            Ok(Some(export_path)) => {
                                let exporter: Singleton<Exporter> = Dependency::get();

                                let directory = export_path.parent().unwrap();
                                let file_name = export_path.file_name().unwrap();

                                match &self.edit {
                                    Some(edit) => {
                                        exporter.with_lock_mut(|exporter| {
                                            exporter.export(
                                                ui.ctx().clone(),
                                                edit.read()
                                                    .unwrap()
                                                    .state
                                                    .pages_state
                                                    .pages
                                                    .values()
                                                    .into_iter()
                                                    .map(|x| x.clone())
                                                    .collect::<Vec<_>>(),
                                                directory.into(),
                                                file_name.to_str().unwrap(),
                                            );
                                        });
                                    }
                                    None => {
                                        // Show alert
                                        ModalManager::push(BasicModal::new(
                                            "Error",
                                            "Nothing to export",
                                            "OK",
                                        ));
                                    }
                                };
                            }
                            Err(e) => {
                                error!("Error opening export file dialog: {:?}", e);
                            }
                            Ok(None) => {
                                info!("No export directory selected");
                            }
                        }
                    }
                });

                ui.menu_button("Group By", |ui| {
                    let photo_manager: Singleton<PhotoManager> = Dependency::get();
                    photo_manager.with_lock_mut(|photo_manager| {
                        if ui.button("Date").clicked() {
                            photo_manager.group_photos_by(PhotoGrouping::Date);
                        }
                        if ui.button("Rating").clicked() {
                            photo_manager.group_photos_by(PhotoGrouping::Rating);
                        }
                    });
                });

                ui.menu_button("Project Settings", |ui| {
                    if ui.button("Page Settings").clicked() {
                        self.page_settings_modal_id =
                            Some(ModalManager::push(PageSettingsModal::new()));
                    }
                });

                ui.menu_button("Debug", |ui| {
                    Dependency::<DebugSettings>::get().with_lock_mut(|debug_settings| {
                        fn enabled_disabled_suffix(enabled: bool) -> &'static str {
                            if enabled { "(Enabled)" } else { "(Disabled)" }
                        }

                        if ui
                            .button(format!(
                                "Quick Layout Numbers:{}",
                                enabled_disabled_suffix(debug_settings.show_quick_layout_order)
                            ))
                            .clicked()
                        {
                            debug_settings.show_quick_layout_order.toggle();
                        }
                    });
                })
            });

            ui.add_space(10.0);

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
                    SceneTransition::_Gallery(scene) => {
                        *self.organize.write().unwrap() = scene;
                        self.show_organize();
                        SceneResponse::None
                    }
                    SceneTransition::_Canvas(mut scene) => {
                        scene.state.gallery_state = self
                            .organize
                            .read()
                            .unwrap()
                            .state
                            .image_gallery_state
                            .clone();
                        self.edit = Some(Arc::new(RwLock::new(scene)));
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

    fn popped(&mut self, popped_scene_response: ScenePopResponse) {
        match self.current {
            Either::Left(ref mut organize) => {
                organize.write().unwrap().popped(popped_scene_response);
            }
            Either::Right(ref mut edit) => {
                edit.write().unwrap().popped(popped_scene_response);
            }
        }
    }
}
