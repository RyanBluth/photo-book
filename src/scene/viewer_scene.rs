use egui::Widget;
use egui_tiles::UiResponse;

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    photo::{Photo, SaveOnDropPhoto},
    photo_manager::PhotoManager,
    widget::{
        image_viewer::{self, ImageViewer, ImageViewerState},
        photo_info::PhotoInfo,
    },
};

use super::{NavigationRequest, Navigator, Scene, SceneResponse};

pub struct ViewerSceneState {
    photo: Photo,
    viewer_state: ImageViewerState,
}

impl ViewerSceneState {
    fn new(photo: Photo) -> Self {
        Self {
            photo,
            viewer_state: ImageViewerState::default(),
        }
    }
}

pub enum ViewerScenePane {
    Viewer,
    PhotoInfo,
}

pub struct ViewerScene {
    state: ViewerSceneState,
    tree: egui_tiles::Tree<ViewerScenePane>,
}

impl ViewerScene {
    pub fn new(photo: Photo) -> Self {
        let mut tiles = egui_tiles::Tiles::default();

        let viewer_id = tiles.insert_pane(ViewerScenePane::Viewer);
        let photo_info_id = tiles.insert_pane(ViewerScenePane::PhotoInfo);

        let children = vec![viewer_id, photo_info_id];

        let mut linear_layout =
            egui_tiles::Linear::new(egui_tiles::LinearDir::Horizontal, children);
        linear_layout.shares.set_share(photo_info_id, 0.2);

        Self {
            state: ViewerSceneState::new(photo),
            tree: egui_tiles::Tree::new(
                "viewer_scene_tree",
                tiles.insert_container(linear_layout),
                tiles,
            ),
        }
    }
}

impl Scene for ViewerScene {
    fn ui(&mut self, ui: &mut egui::Ui) -> SceneResponse {
        let mut navigator = Navigator::new();

        self.tree.ui(
            &mut ViewerTreeBehavior {
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

struct ViewerTreeBehavior<'a> {
    scene_state: &'a mut ViewerSceneState,
    navigator: &'a mut Navigator,
}

impl<'a> egui_tiles::Behavior<ViewerScenePane> for ViewerTreeBehavior<'a> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut ViewerScenePane,
    ) -> UiResponse {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();

        match pane {
            ViewerScenePane::Viewer => {
                let viewer_response =
                    ImageViewer::new(&self.scene_state.photo, &mut self.scene_state.viewer_state)
                        .show(ui);
                if let Some(request) = viewer_response.request { match request {
                    image_viewer::Request::Exit => {
                        self.navigator.pop();
                    }
                    image_viewer::Request::Previous => {
                        photo_manager.with_lock_mut(|photo_manager| {
                            let (prev_photo, new_index) = photo_manager
                                .previous_photo(&self.scene_state.photo, ui.ctx())
                                .unwrap()
                                .unwrap();

                            self.scene_state.photo = prev_photo;
                            self.scene_state.viewer_state = ImageViewerState::default();
                        });
                    }
                    image_viewer::Request::Next => {
                        photo_manager.with_lock_mut(|photo_manager| {
                            let (next_photo, new_index) = photo_manager
                                .next_photo(&self.scene_state.photo, ui.ctx())
                                .unwrap()
                                .unwrap();

                            self.scene_state.photo = next_photo;
                            self.scene_state.viewer_state = ImageViewerState::default();
                        });
                    }
                } }
            }
            ViewerScenePane::PhotoInfo => {
                ui.set_max_width(600.0);
                PhotoInfo::new(SaveOnDropPhoto::new(&mut self.scene_state.photo)).show(ui);
            }
        }

        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &ViewerScenePane) -> egui::WidgetText {
        match pane {
            ViewerScenePane::Viewer => "Viewer".into(),
            ViewerScenePane::PhotoInfo => "Photo Info".into(),
        }
    }
}
