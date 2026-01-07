use crop_scene::{CropScene, CropSceneResponse};

use crate::{
    photo::Photo,
    widget::{
        canvas::CanvasState, image_gallery::ImageGalleryState, image_viewer::ImageViewerState,
    },
};

use self::{
    canvas_scene::CanvasScene, organize_edit_scene::OrganizeEditScene,
    organize_scene::GalleryScene, viewer_scene::ViewerScene,
};

pub mod canvas_scene;
pub mod crop_scene;
pub mod organize_edit_scene;
pub mod organize_scene;
pub mod viewer_scene;

pub enum SceneResponse {
    None,
    Pop(ScenePopResponse),
    Push(SceneTransition),
}

pub enum ScenePopResponse {
    None,
    Crop(CropSceneResponse),
}

pub enum SceneTransition {
    _OrganizeEdit(OrganizeEditScene),
    _Gallery(GalleryScene),
    Viewer(ViewerScene),
    _Canvas(CanvasScene),
    Crop(CropScene),
}

impl SceneTransition {
    pub fn scene(self) -> Box<dyn Scene> {
        match self {
            SceneTransition::_OrganizeEdit(scene) => Box::new(scene),
            SceneTransition::_Gallery(scene) => Box::new(scene),
            SceneTransition::Viewer(scene) => Box::new(scene),
            SceneTransition::_Canvas(scene) => Box::new(scene),
            SceneTransition::Crop(scene) => Box::new(scene),
        }
    }
}

impl PartialEq for SceneTransition {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SceneTransition::_OrganizeEdit(_), SceneTransition::_OrganizeEdit(_)) => true,
            (SceneTransition::_Gallery(_), SceneTransition::_Gallery(_)) => true,
            (SceneTransition::Viewer(_), SceneTransition::Viewer(_)) => true,
            (SceneTransition::_Canvas(_), SceneTransition::_Canvas(_)) => true,
            (SceneTransition::Crop(_), SceneTransition::Crop(_)) => true,
            _ => false,
        }
    }
}

pub trait Scene: Send + Sync {
    fn ui(&mut self, ui: &mut egui::Ui) -> SceneResponse;

    fn popped(&mut self, _popped_scene_response: ScenePopResponse) {}
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

    pub fn _swap(&mut self, scene: SceneTransition) {
        self.scenes.pop();
        self.scenes.push(scene.scene());
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let response = match self.scenes.last_mut() {
            Some(scene) => scene.ui(ui),
            _ => self.root_scene.ui(ui),
        };

        match response {
            SceneResponse::None => {}
            SceneResponse::Pop(response) => {
                self.pop();

                match self.scenes.last_mut() {
                    Some(scene) => scene.popped(response),
                    None => self.root_scene.popped(response),
                }
            }
            SceneResponse::Push(scene) => {
                self.push(scene);
            }
        }
    }

    pub fn _root_scene(&self) -> &OrganizeEditScene {
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
    Pop(ScenePopResponse),
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

    pub fn pop(&mut self, response: ScenePopResponse) {
        self.request = Some(NavigationRequest::Pop(response));
    }

    pub fn process_pending_request(self) -> Option<NavigationRequest> {
        self.request
    }
}

pub struct _CanvasSceneState {
    canvas_state: CanvasState,
}

impl _CanvasSceneState {
    fn _new(canvas_state: CanvasState) -> Self {
        Self { canvas_state }
    }
}

pub enum _SceneState {
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
