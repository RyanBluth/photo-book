use std::collections::HashSet;

use egui::{menu, Color32, Widget};
use egui_tiles::UiResponse;
use log::info;

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    photo_manager::{PhotoLoadResult, PhotoManager},
    widget::{
        image_gallery::{ImageGallery, ImageGalleryResponse, ImageGalleryState},
        photo_info::PhotoInfo,
    },
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
                scale: 1.0,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GalleryScenePane {
    Gallery,
    PhotoInfo,
}

#[derive(Debug, Clone)]
pub struct GalleryScene {
    pub state: GallerySceneState,
    tree: egui_tiles::Tree<GalleryScenePane>,
}

impl GalleryScene {
    pub fn new() -> Self {
        let mut tiles = egui_tiles::Tiles::default();

        let gallery_pane_id = tiles.insert_pane(GalleryScenePane::Gallery);
        let info_pane_id = tiles.insert_pane(GalleryScenePane::PhotoInfo);

        let mut linear_layout = egui_tiles::Linear::new(
            egui_tiles::LinearDir::Horizontal,
            vec![gallery_pane_id, info_pane_id],
        );

        linear_layout.shares.set_share(info_pane_id, 0.2);

        Self {
            state: GallerySceneState::default(),
            tree: egui_tiles::Tree::new(
                "organize_scene_tree",
                tiles.insert_container(linear_layout),
                tiles,
            ),
        }
    }
}

impl Scene for GalleryScene {
    fn ui(&mut self, ui: &mut egui::Ui) -> SceneResponse {

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
                        ImageGalleryResponse::ViewPhoto(photo) => {
                            photo_manager.with_lock(|photo_manager| {

                                self.navigator
                                    .push(SceneTransition::Viewer(ViewerScene::new(photo)));
                            });
                        }
                        ImageGalleryResponse::EditPhotoAt(index) => {
                            photo_manager.with_lock(|photo_manager| {
                                // TODO: Allow clicking on a pending photo
                                let photo = photo_manager.photos[index].clone();

                                self.navigator.push(SceneTransition::Canvas(
                                    CanvasScene::with_photo(
                                        photo,
                                        Some(self.scene_state.image_gallery_state.clone()),
                                    ),
                                ));
                            });
                        }
                    }
                }
            }
            GalleryScenePane::PhotoInfo => {
                let photo_manager: Singleton<PhotoManager> = Dependency::get();

                let gallery_state = &self.scene_state.image_gallery_state;

                if let Some(selected_image) = gallery_state.selected_images.iter().next() {
                    photo_manager.with_lock(|photo_manager| {
                        let photo = photo_manager.photos[selected_image].clone();

                        PhotoInfo::new(&photo).ui(ui);
                    });
                }
            }
        }

        UiResponse::None
    }
}
