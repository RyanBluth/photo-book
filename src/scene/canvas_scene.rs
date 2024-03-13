use egui::Widget;
use egui_tiles::UiResponse;
use font_kit::canvas::Canvas;

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    photo::Photo,
    photo_manager::PhotoManager,
    widget::{
        image_gallery::ImageGalleryState,
        image_viewer::{self, ImageViewer, ImageViewerState},
        page_canvas::CanvasState,
        photo_info::PhotoInfo,
    },
};

use super::{NavigationRequest, Navigator, Scene, SceneResponse};

pub struct CanvasSceneState {
    canvas_state: CanvasState,
}

impl CanvasSceneState {
    fn new() -> Self {
        Self {
            canvas_state: CanvasState::new(),
        }
    }

    fn with_photo(photo: Photo) -> Self {
        Self {
            canvas_state: CanvasState::with_photo(photo, ImageGalleryState::default()),
        }
    }
}

pub enum CanvasScenePane {
    Gallery,
    Canvas,
    Info,
}

pub struct CanvasScene {
    state: CanvasSceneState,
    tree: egui_tiles::Tree<CanvasScenePane>,
}

impl CanvasScene {
    pub fn with_photo(photo: Photo) -> Self {
        let mut tiles = egui_tiles::Tiles::default();

        let gallery_id = tiles.insert_pane(CanvasScenePane::Gallery);
        let canvas_id = tiles.insert_pane(CanvasScenePane::Canvas);
        let info_id = tiles.insert_pane(CanvasScenePane::Info);

        let children = vec![gallery_id, canvas_id, info_id];

        let mut linear_layout =
            egui_tiles::Linear::new(egui_tiles::LinearDir::Horizontal, children);
        linear_layout.shares.set_share(gallery_id, 0.2);
        linear_layout.shares.set_share(info_id, 0.2);

        Self {
            state: CanvasSceneState::with_photo(photo),
            tree: egui_tiles::Tree::new(
                "canvas_scene_tree",
                tiles.insert_container(linear_layout),
                tiles,
            ),
        }
    }
}

impl Scene for CanvasScene {
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
    scene_state: &'a mut CanvasSceneState,
    navigator: &'a mut Navigator,
}

impl<'a> egui_tiles::Behavior<CanvasScenePane> for ViewerTreeBehavior<'a> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut CanvasScenePane,
    ) -> UiResponse {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();

        match pane {
            CanvasScenePane::Gallery => todo!(),
            CanvasScenePane::Canvas => todo!(),
            CanvasScenePane::Info => todo!(),
        }

        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &CanvasScenePane) -> egui::WidgetText {
        match pane {
            CanvasScenePane::Gallery => "Gallery".into(),
            CanvasScenePane::Canvas => "Canvas".into(),
            CanvasScenePane::Info => "Info".into(),
        }
    }
}
