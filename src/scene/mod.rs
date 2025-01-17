use crate::{
    photo::Photo,
    widget::{
        image_gallery::ImageGalleryState, image_viewer::ImageViewerState, page_canvas::CanvasState,
    },
};

use self::{
    canvas_scene::CanvasScene, organize_edit_scene::OrganizeEditScene,
    organize_scene::GalleryScene, viewer_scene::ViewerScene,
};

pub mod canvas_scene;
pub mod organize_edit_scene;
pub mod organize_scene;
pub mod viewer_scene;

pub enum SceneResponse {
    None,
    Pop,
    Push(SceneTransition),
}

pub enum SceneTransition {
    OrganizeEdit(OrganizeEditScene),
    Gallery(GalleryScene),
    Viewer(ViewerScene),
    Canvas(CanvasScene),
}

impl SceneTransition {
    pub fn scene(self) -> Box<dyn Scene> {
        match self {
            SceneTransition::OrganizeEdit(scene) => Box::new(scene),
            SceneTransition::Gallery(scene) => Box::new(scene),
            SceneTransition::Viewer(scene) => Box::new(scene),
            SceneTransition::Canvas(scene) => Box::new(scene),
        }
    }
}

impl PartialEq for SceneTransition {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SceneTransition::OrganizeEdit(_), SceneTransition::OrganizeEdit(_)) => true,
            (SceneTransition::Gallery(_), SceneTransition::Gallery(_)) => true,
            (SceneTransition::Viewer(_), SceneTransition::Viewer(_)) => true,
            (SceneTransition::Canvas(_), SceneTransition::Canvas(_)) => true,
            _ => false,
        }
    }
}

pub trait Scene: Send + Sync {
    fn ui(&mut self, ui: &mut egui::Ui) -> SceneResponse;
}

pub struct SceneManager {
    pub root_scene: OrganizeEditScene,
    scenes: Vec<Box<dyn Scene>>,
}

impl SceneManager {
    pub fn new(root_scene: OrganizeEditScene) -> Self {
        Self {
            root_scene,
            scenes: vec![],
        }
    }

    pub fn push(&mut self, scene: SceneTransition) {
        self.scenes.push(scene.scene());
    }

    pub fn pop(&mut self) {
        self.scenes.pop();
    }

    pub fn swap(&mut self, scene: SceneTransition) {
        self.scenes.pop();
        self.scenes.push(scene.scene());
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let response = if let Some(scene) = self.scenes.last_mut() {
            scene.ui(ui)
        } else {
            self.root_scene.ui(ui)
        };

        match response {
            SceneResponse::None => {}
            SceneResponse::Pop => {
                self.pop();
            }
            SceneResponse::Push(scene) => {
                self.push(scene);
            }
        }
    }

    pub fn root_scene(&self) -> &OrganizeEditScene {
        &self.root_scene
    }
}

impl Default for SceneManager {
    fn default() -> Self {
        Self {
            root_scene: OrganizeEditScene::new(GalleryScene::new(), None),
            scenes: vec![],
        }
    }
}

pub enum NavigationRequest {
    Push(SceneTransition),
    Pop,
}

pub struct Navigator {
    request: Option<NavigationRequest>,
}

impl Navigator {
    pub fn new() -> Self {
        Self { request: None }
    }

    pub fn push(&mut self, scene: SceneTransition) {
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
