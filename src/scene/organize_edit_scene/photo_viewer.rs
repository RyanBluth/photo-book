use std::sync::{Arc, RwLock};

use egui_tiles::TileId;

use super::super::viewer_scene::ViewerScene;

pub type PhotoViewerId = String;

#[derive(Debug, Clone)]
pub(super) struct OpenPhotoViewer {
    pub(super) viewer_id: PhotoViewerId,
    pub(super) scene: Arc<RwLock<ViewerScene>>,
    pub(super) tile_id: TileId,
}
