use std::sync::{Arc, RwLock};

use egui_tiles::TileId;

use crate::widget::book_list::BookListEntry;

use super::super::canvas_scene::{CanvasScene, CanvasSceneState};

pub type BookId = String;

#[derive(Debug, Clone)]
pub(super) struct OpenBookEditor {
    pub(super) book_id: BookId,
    pub(super) scene: Arc<RwLock<CanvasScene>>,
    pub(super) tile_id: TileId,
}

#[derive(Debug, Clone)]
pub struct Book {
    pub id: BookId,
    pub name: String,
    pub state: CanvasSceneState,
}

impl Book {
    pub fn new(name: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            state: CanvasSceneState::new(),
        }
    }

    pub fn with_state(id: BookId, name: String, state: CanvasSceneState) -> Self {
        Self { id, name, state }
    }

    fn page_count(&self) -> usize {
        self.state.pages_state.pages.len()
    }

    pub(super) fn list_entry(&self) -> BookListEntry {
        BookListEntry {
            id: self.id.clone(),
            name: self.name.clone(),
            page_count: self.page_count(),
        }
    }
}
