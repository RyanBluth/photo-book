use egui_tiles::UiResponse;

use crate::{
    dep_mut,
    photo::{Photo, SaveOnDropPhoto},
    photo_manager::PhotoManager,
    widget::{
        image_viewer::{self, ImageViewer, ImageViewerState},
        photo_filmstrip::{PhotoFilmstrip, PhotoFilmstripState},
        photo_info::{PhotoInfo, PhotoInfoState},
    },
};

use super::{NavigationRequest, Navigator, Scene, ScenePopResponse, SceneResponse};

pub struct ViewerSceneState {
    photo: Photo,
    viewer_state: ImageViewerState,
    photo_filmstrip_state: PhotoFilmstripState,
    photo_info_state: PhotoInfoState,
    was_paging_key_down: bool,
}

impl ViewerSceneState {
    fn new(photo: Photo) -> Self {
        Self {
            photo,
            viewer_state: ImageViewerState::default(),
            photo_filmstrip_state: PhotoFilmstripState::default(),
            photo_info_state: PhotoInfoState::new(),
            was_paging_key_down: false,
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

        let filmstrip_response =
            PhotoFilmstrip::new(&self.state.photo, &mut self.state.photo_filmstrip_state).show(ui);
        if let Some(photo) = filmstrip_response.selected_photo {
            if photo.path != self.state.photo.path {
                self.state.photo = photo;
                self.state.viewer_state = ImageViewerState::default();
            }
        }

        self.tree.ui(
            &mut ViewerTreeBehavior {
                scene_state: &mut self.state,
                navigator: &mut navigator,
            },
            ui,
        );

        let paging_key_down = ui.input(|input| {
            input.key_down(egui::Key::ArrowLeft) || input.key_down(egui::Key::ArrowRight)
        });
        let is_holding_paging_key = paging_key_down && self.state.was_paging_key_down;
        self.state.was_paging_key_down = paging_key_down;

        if !is_holding_paging_key {
            dep_mut!(PhotoManager, |photo_manager| {
                let _ = photo_manager.preload_surrounding_textures(&self.state.photo, ui.ctx());
            });
        }

        match navigator.process_pending_request() {
            Some(NavigationRequest::Push(scene_state)) => SceneResponse::Push(scene_state),
            Some(NavigationRequest::Pop(response)) => SceneResponse::Pop(response),
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
        match pane {
            ViewerScenePane::Viewer => {
                let viewer_response =
                    ImageViewer::new(&self.scene_state.photo, &mut self.scene_state.viewer_state)
                        .show(ui);
                if let Some(request) = viewer_response.request {
                    match request {
                        image_viewer::Request::Exit => {
                            self.navigator.pop(ScenePopResponse::None);
                        }
                        image_viewer::Request::Previous => {
                            dep_mut!(PhotoManager, |photo_manager| {
                                if let Some(prev_photo) = photo_manager
                                    .previous_photo(&self.scene_state.photo, ui.ctx())
                                    .unwrap()
                                {
                                    self.scene_state.photo = prev_photo;
                                    self.scene_state.viewer_state = ImageViewerState::default();
                                }
                            });
                        }
                        image_viewer::Request::Next => {
                            dep_mut!(PhotoManager, |photo_manager| {
                                if let Some(next_photo) = photo_manager
                                    .next_photo(&self.scene_state.photo, ui.ctx())
                                    .unwrap()
                                {
                                    self.scene_state.photo = next_photo;
                                    self.scene_state.viewer_state = ImageViewerState::default();
                                }
                            });
                        }
                    }
                }
            }
            ViewerScenePane::PhotoInfo => {
                ui.set_max_width(600.0);
                PhotoInfo::new(
                    SaveOnDropPhoto::new(&mut self.scene_state.photo),
                    &mut self.scene_state.photo_info_state,
                )
                .show(ui);
            }
        }

        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &ViewerScenePane) -> egui::widget_text::WidgetText {
        match pane {
            ViewerScenePane::Viewer => "Viewer".into(),
            ViewerScenePane::PhotoInfo => "Photo Info".into(),
        }
    }
}
