use eframe::wgpu::Label;

use crate::{
    photo::Photo,
    widget::{
        image_gallery::ImageGalleryState, image_viewer::ImageViewerState, page_canvas::CanvasState,
    },
};
use std::collections::HashSet;

use self::organize_scene::GalleryScene;

pub mod organize_scene;
pub mod viewer_scene;

enum SceneResponse {
    None,
    Pop,
    Push(SceneState),
}

trait Scene: Send + Sync {
    fn ui(&mut self, ui: &mut egui::Ui) -> SceneResponse;
}

struct Test {}
impl<'a> Scene for Test {
    fn ui(&mut self, ui: &mut egui::Ui) -> SceneResponse {
        ui.label("Hello");
        SceneResponse::None
    }
}

pub struct SceneManager {
    scenes: Vec<SceneState>,
    scene: Option<Box<dyn Scene>>,
}

impl SceneManager {
    pub fn empty() -> Self {
        Self {
            scenes: vec![],
            scene: None,
        }
    }

    pub fn push(&mut self, scene: SceneState) {
        self.scenes.push(scene);
        self.update_active_scene();
    }

    pub fn pop(&mut self) -> Option<SceneState> {
        if self.scenes.len() <= 1 {
            return None;
        }
        let res = self.scenes.pop();
        self.update_active_scene();
        res
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if let Some(scene) = self.scene.as_mut() {
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

    fn update_active_scene(&mut self) {
        self.scene = match self.scenes.last() {
            Some(SceneState::Gallery { state }) => Some(Box::new(GalleryScene::new())),
            Some(SceneState::Viewer {
                photo,
                index,
                state,
            }) => Some(Box::new(viewer_scene::ViewerScene::new(
                photo.clone(),
                *index,
            ))),
            Some(SceneState::Canvas { state }) => {
                // todo!();
                Some(Box::new(Test {}))
            }
            None => None,
        };
    }
}

impl Default for SceneManager {
    fn default() -> Self {
        let mut res = Self {
            scenes: vec![SceneState::Gallery {
                state: ImageGalleryState {
                    selected_images: HashSet::new(),
                    current_dir: None,
                },
            }],
            scene: None,
        };
        res.update_active_scene();
        res
    }
}

pub enum NavigationRequest {
    Push(SceneState),
    Pop,
}

pub struct Navigator {
    request: Option<NavigationRequest>,
}

impl Navigator {
    pub fn new() -> Self {
        Self { request: None }
    }

    pub fn push(&mut self, scene: SceneState) {
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
