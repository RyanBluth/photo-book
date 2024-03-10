use std::sync::Arc;

use egui::menu;
use egui_tiles::UiResponse;
use log::info;

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    photo_manager::{PhotoLoadResult, PhotoManager},
    widget::{
        image_gallery::{ImageGallery, ImageGalleryResponse, ImageGalleryState},
        image_viewer::ImageViewerState,
        page_canvas::CanvasState,
    },
    NavAction, PrimaryComponent,
};

use super::{
    GallerySceneState, NavigationRequest, Navigator, Scene, SceneManager, SceneResponse, SceneState,
};

pub enum GalleryScenePane {
    Gallery,
}

pub struct GalleryScene {
    state: GallerySceneState,
    tree: egui_tiles::Tree<GalleryScenePane>,
}

impl GalleryScene {
    pub fn new() -> Self {
        let mut tiles = egui_tiles::Tiles::default();

        let gallery = GalleryScenePane::Gallery;

        let mut tabs = vec![];

        tabs.push(tiles.insert_pane(gallery));

        Self {
            state: GallerySceneState::new(),
            tree: egui_tiles::Tree::new("root_tree", tiles.insert_tab_tile(tabs), tiles),
        }
    }
}

struct GalleryTreeBehavior<'a> {
    scene_state: &'a mut GallerySceneState,
    navigator: &'a mut Navigator,
}

impl<'a> egui_tiles::Behavior<GalleryScenePane> for GalleryTreeBehavior<'a> {
    fn tab_title_for_pane(&mut self, component: &GalleryScenePane) -> egui::WidgetText {
        "Gallery".into()
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        component: &mut GalleryScenePane,
    ) -> egui_tiles::UiResponse {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        let mut nav_action = None;

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
                                    nav_action = Some(NavAction::Push(PrimaryComponent::Viewer {
                                        photo: photo.clone(),
                                        index,
                                        state: ImageViewerState::default(),
                                    }))
                                }
                            });
                        }
                        ImageGalleryResponse::EditPhotoAt(index) => {
                            photo_manager.with_lock(|photo_manager| {
                                // TODO: Allow clicking on a pending photo
                                if let PhotoLoadResult::Ready(photo) =
                                    photo_manager.photos[index].clone()
                                {
                                    self.navigator.push(SceneState::Canvas {
                                        state: CanvasState::with_photo(
                                            photo,
                                            ImageGalleryState::default(),
                                        ),
                                    });

                                    // let gallery_state = match component {
                                    //     PrimaryComponent::Gallery { state } => state.clone(),
                                    //     _ => ImageGalleryState::default(),
                                    // };

                                    // nav_action = Some(NavAction::Push(PrimaryComponent::Canvas {
                                    //     state: CanvasState::with_photo(photo, gallery_state),
                                    // }));
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
            Some(NavigationRequest::Push(scene_state)) => SceneResponse::Push(scene_state),
            Some(NavigationRequest::Pop) => SceneResponse::Pop,
            None => SceneResponse::None,
        }
    }
}
