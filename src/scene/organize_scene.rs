use std::collections::HashSet;

use egui::{menu, Color32};
use egui_tiles::UiResponse;
use log::info;

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    photo_manager::{PhotoLoadResult, PhotoManager},
    widget::image_gallery::{ImageGallery, ImageGalleryResponse, ImageGalleryState},
};

use super::{
    canvas_scene::CanvasScene, viewer_scene::ViewerScene, NavigationRequest, Navigator, Scene,
    SceneResponse, SceneTransition,
};

#[derive(Debug, Clone)]
pub struct GallerySceneState {
    pub image_gallery_state: ImageGalleryState,
}

impl Default for GallerySceneState {
    fn default() -> Self {
        Self {
            image_gallery_state: ImageGalleryState {
                selected_images: HashSet::new(),
                current_dir: None,
                scale: 0.5,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GalleryScenePane {
    Gallery,
}

#[derive(Debug, Clone)]
pub struct GalleryScene {
    pub state: GallerySceneState,
    tree: egui_tiles::Tree<GalleryScenePane>,
}

impl GalleryScene {
    pub fn new() -> Self {
        let mut tiles = egui_tiles::Tiles::default();

        let gallery = GalleryScenePane::Gallery;

        let mut tabs = vec![];

        tabs.push(tiles.insert_pane(gallery));

        Self {
            state: GallerySceneState::default(),
            tree: egui_tiles::Tree::new("organize_scene_tree", tiles.insert_tab_tile(tabs), tiles),
        }
    }
}

impl Scene for GalleryScene {
    fn ui(&mut self, ui: &mut egui::Ui) -> SceneResponse {
        let ref mut current_dir = self.state.image_gallery_state.current_dir;

        menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open").clicked() {
                    *current_dir = native_dialog::FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg"])
                        .show_open_single_dir()
                        .unwrap();

                    info!("Opened {:?}", current_dir);

                    PhotoManager::load_directory(current_dir.clone().unwrap());
                }
            });

            // Temp way to go between gallery and pages
            ui.menu_button("View", |ui| if ui.button("Gallery").clicked() {});
        });

        let mut navigator = Navigator::new();

        self.tree.ui(
            &mut GalleryTreeBehavior {
                scene_state: &mut self.state,
                navigator: &mut navigator,
            },
            ui,
        );

        match navigator.process_pending_request() {
            Some(NavigationRequest::Push(scene)) => SceneResponse::Push(scene),
            Some(NavigationRequest::Pop) => SceneResponse::Pop,
            None => SceneResponse::None,
        }
    }
}

struct GalleryTreeBehavior<'a> {
    scene_state: &'a mut GallerySceneState,
    navigator: &'a mut Navigator,
}

impl<'a> egui_tiles::Behavior<GalleryScenePane> for GalleryTreeBehavior<'a> {
    fn tab_title_for_pane(&mut self, _pane: &GalleryScenePane) -> egui::WidgetText {
        "Gallery".into()
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        component: &mut GalleryScenePane,
    ) -> egui_tiles::UiResponse {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();

        match component {
            GalleryScenePane::Gallery => {
                let gallery_response =
                    ImageGallery::show(ui, &mut self.scene_state.image_gallery_state);

                if let Some(gallery_response) = gallery_response {
                    match gallery_response {
                        ImageGalleryResponse::ViewPhotoAt(index) => {
                            photo_manager.with_lock(|photo_manager| {
                                // TODO: Allow clicking on a pending photo
                                if let PhotoLoadResult::Ready(photo) =
                                    photo_manager.photos[index].clone()
                                {
                                    self.navigator
                                        .push(SceneTransition::Viewer(ViewerScene::new(
                                            photo, index,
                                        )));
                                }
                            });
                        }
                        ImageGalleryResponse::EditPhotoAt(index) => {
                            photo_manager.with_lock(|photo_manager| {
                                // TODO: Allow clicking on a pending photo
                                if let PhotoLoadResult::Ready(photo) =
                                    photo_manager.photos[index].clone()
                                {
                                    self.navigator.push(SceneTransition::Canvas(
                                        CanvasScene::with_photo(
                                            photo,
                                            Some(self.scene_state.image_gallery_state.clone()),
                                        ),
                                    ));
                                }
                            });
                        }
                    }
                }
            }
        }

        UiResponse::None
    }
}
