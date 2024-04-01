use crate::{
    photo::Photo,
    widget::{
        image_gallery::ImageGalleryState, image_viewer::ImageViewerState, page_canvas::CanvasState,
    },
};

use self::organize_scene::GalleryScene;

pub mod canvas_scene;
pub mod organize_scene;
pub mod viewer_scene;

pub enum SceneResponse {
    None,
    Pop,
    Push(Box<dyn Scene>),
}

pub trait Scene: Send + Sync {
    fn ui(&mut self, ui: &mut egui::Ui) -> SceneResponse;
}

pub struct SceneManager {
    scenes: Vec<Box<dyn Scene>>,
}

impl SceneManager {
    pub fn empty() -> Self {
        Self { scenes: vec![] }
    }

    pub fn push(&mut self, scene: Box<dyn Scene>) {
        self.scenes.push(scene);
    }

    pub fn pop(&mut self) {
        self.scenes.pop();
    }

    pub fn swap(&mut self, scene: Box<dyn Scene>) {
        self.scenes.pop();
        self.scenes.push(scene);
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if let Some(scene) = self.scenes.last_mut() {
            match scene.ui(ui) {
                SceneResponse::None => {}
                SceneResponse::Pop => {
                    self.pop();
                }
                SceneResponse::Push(scene) => {
                    self.push(scene);
                }
            }
        }
    }
}

impl Default for SceneManager {
    fn default() -> Self {
        Self {
            scenes: vec![Box::new(GalleryScene::new())],
        }
    }
}

pub enum NavigationRequest {
    Push(Box<dyn Scene>),
    Pop,
}

pub struct Navigator {
    request: Option<NavigationRequest>,
}

impl Navigator {
    pub fn new() -> Self {
        Self { request: None }
    }

    pub fn push(&mut self, scene: Box<dyn Scene>) {
        self.request = Some(NavigationRequest::Push(scene));
    }

    pub fn pop(&mut self) {
        self.request = Some(NavigationRequest::Pop);
    }

    pub fn process_pending_request(self) -> Option<NavigationRequest> {
        self.request
    }
}

pub struct CanvasSceneState {
    canvas_state: CanvasState,
}

impl CanvasSceneState {
    fn new(canvas_state: CanvasState) -> Self {
        Self { canvas_state }
    }
}

pub enum SceneState {
    Gallery {
        state: ImageGalleryState,
    },
    Viewer {
        photo: Photo,
        index: usize,
        state: ImageViewerState,
    },
    Canvas {
        state: CanvasState,
    },
}
