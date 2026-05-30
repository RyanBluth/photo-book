use egui::Ui;
use egui_tiles::{
    Behavior as TileBehavior, Container, SimplificationOptions, Tile, TileId, Tiles, Tree,
    UiResponse as TileUiResponse,
};

use crate::scene::Scene;

use super::{BookId, OrganizeEditScene, SceneResponse};

#[derive(Debug, Clone, PartialEq)]
pub(super) enum WorkspacePane {
    Gallery,
    Book(BookId),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum WorkspaceAction {
    CreateBook,
    SelectBook(BookId),
}

impl OrganizeEditScene {
    pub(super) fn initial_workspace_tabs() -> Tree<WorkspacePane> {
        let mut tiles = Tiles::default();
        let gallery_tile = tiles.insert_pane(WorkspacePane::Gallery);
        let root = tiles.insert_tab_tile(vec![gallery_tile]);
        Tree::new("collection_workspace_tabs", root, tiles)
    }

    pub(super) fn insert_book_tab(&mut self, book_id: BookId) -> TileId {
        if self.workspace_tabs.root.is_none() {
            self.workspace_tabs = Self::initial_workspace_tabs();
        }

        let tile_id = self
            .workspace_tabs
            .tiles
            .insert_pane(WorkspacePane::Book(book_id));

        let Some(root) = self.workspace_tabs.root else {
            return tile_id;
        };

        match self.workspace_tabs.tiles.get_mut(root) {
            Some(Tile::Container(Container::Tabs(tabs))) => {
                tabs.add_child(tile_id);
                tabs.set_active(tile_id);
            }
            Some(_) => {
                let tabs_root = self
                    .workspace_tabs
                    .tiles
                    .insert_tab_tile(vec![root, tile_id]);
                if let Some(Tile::Container(Container::Tabs(tabs))) =
                    self.workspace_tabs.tiles.get_mut(tabs_root)
                {
                    tabs.set_active(tile_id);
                }
                self.workspace_tabs.root = Some(tabs_root);
            }
            None => {
                let gallery_tile = self
                    .workspace_tabs
                    .tiles
                    .insert_pane(WorkspacePane::Gallery);
                let tabs_root = self
                    .workspace_tabs
                    .tiles
                    .insert_tab_tile(vec![gallery_tile, tile_id]);
                if let Some(Tile::Container(Container::Tabs(tabs))) =
                    self.workspace_tabs.tiles.get_mut(tabs_root)
                {
                    tabs.set_active(tile_id);
                }
                self.workspace_tabs.root = Some(tabs_root);
            }
        }

        tile_id
    }

    pub(super) fn activate_gallery_tab(&mut self) -> bool {
        self.workspace_tabs
            .make_active(|_, tile| matches!(tile, Tile::Pane(WorkspacePane::Gallery)))
    }

    pub(super) fn activate_book_tab(&mut self, book_id: &str) -> bool {
        self.workspace_tabs.make_active(|_, tile| {
            matches!(tile, Tile::Pane(WorkspacePane::Book(active_book_id)) if active_book_id.as_str() == book_id)
        })
    }

    pub(super) fn close_book_tab(
        &mut self,
        workspace_tabs: &mut Tree<WorkspacePane>,
        tile_id: TileId,
        book_id: &str,
    ) {
        self.persist_book_state(book_id);
        self.open_books
            .retain(|open_book| open_book.tile_id != tile_id);
        workspace_tabs.remove_recursively(tile_id);
    }

    fn active_workspace_pane(&self) -> Option<WorkspacePane> {
        self.workspace_tabs
            .active_tiles()
            .into_iter()
            .find_map(|tile_id| match self.workspace_tabs.tiles.get(tile_id) {
                Some(Tile::Pane(pane)) => Some(pane.clone()),
                _ => None,
            })
    }

    pub(super) fn sync_active_workspace_tab(&mut self) {
        match self.active_workspace_pane() {
            Some(WorkspacePane::Gallery) => {
                if self.selected_book_id.is_some() {
                    self.persist_active_book_state();
                    self.sync_organize_gallery_from_edit();
                }
                self.selected_book_id = None;
            }
            Some(WorkspacePane::Book(book_id)) => {
                if self.selected_book_id.as_deref() != Some(book_id.as_str()) {
                    self.persist_active_book_state();
                }

                if self
                    .open_books
                    .iter()
                    .any(|open_book| open_book.book_id == book_id)
                {
                    self.selected_book_id = Some(book_id);
                } else {
                    self.select_book(&book_id);
                }
            }
            None => {
                self.activate_gallery_tab();
                self.selected_book_id = None;
            }
        }
    }

    pub(super) fn workspace_tabs_ui(&mut self, ui: &mut Ui) -> SceneResponse {
        let mut workspace_tabs =
            std::mem::replace(&mut self.workspace_tabs, Self::initial_workspace_tabs());
        let mut content_response = None;
        let mut workspace_action = None;
        let mut tab_close_request = None;

        {
            let mut behavior = WorkspaceTabsBehavior {
                scene: self,
                scene_response: &mut content_response,
                workspace_action: &mut workspace_action,
                tab_close_request: &mut tab_close_request,
            };
            workspace_tabs.ui(&mut behavior, ui);
        }

        if let Some((tile_id, book_id)) = tab_close_request {
            self.close_book_tab(&mut workspace_tabs, tile_id, &book_id);
        }

        self.workspace_tabs = workspace_tabs;
        self.sync_active_workspace_tab();

        if let Some(action) = workspace_action {
            self.apply_workspace_action(action);
        }

        self.handle_child_scene_response(content_response.unwrap_or(SceneResponse::None))
    }
}

struct WorkspaceTabsBehavior<'a> {
    scene: &'a mut OrganizeEditScene,
    scene_response: &'a mut Option<SceneResponse>,
    workspace_action: &'a mut Option<WorkspaceAction>,
    tab_close_request: &'a mut Option<(TileId, BookId)>,
}

impl WorkspaceTabsBehavior<'_> {
    fn book_name(&self, book_id: &str) -> String {
        self.scene
            .books
            .iter()
            .find(|book| book.id.as_str() == book_id)
            .map(|book| book.name.clone())
            .unwrap_or_else(|| "Book".to_string())
    }
}

impl TileBehavior<WorkspacePane> for WorkspaceTabsBehavior<'_> {
    fn pane_ui(
        &mut self,
        ui: &mut Ui,
        _tile_id: TileId,
        pane: &mut WorkspacePane,
    ) -> TileUiResponse {
        match pane {
            WorkspacePane::Gallery => {
                let (response, action) = self.scene.collection_mode_ui(ui);
                *self.scene_response = Some(response);
                if let Some(action) = action {
                    *self.workspace_action = Some(action);
                }
            }
            WorkspacePane::Book(book_id) => {
                let response = self
                    .scene
                    .open_books
                    .iter_mut()
                    .find(|open_book| open_book.book_id.as_str() == book_id.as_str())
                    .map(|open_book| open_book.scene.write().unwrap().ui(ui))
                    .unwrap_or(SceneResponse::None);

                *self.scene_response = Some(response);
            }
        }

        TileUiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &WorkspacePane) -> egui::WidgetText {
        match pane {
            WorkspacePane::Gallery => "Gallery".into(),
            WorkspacePane::Book(book_id) => self.book_name(book_id).into(),
        }
    }

    fn is_tab_closable(&self, tiles: &Tiles<WorkspacePane>, tile_id: TileId) -> bool {
        matches!(tiles.get_pane(&tile_id), Some(WorkspacePane::Book(_)))
    }

    fn on_tab_close(&mut self, tiles: &mut Tiles<WorkspacePane>, tile_id: TileId) -> bool {
        let Some(WorkspacePane::Book(book_id)) = tiles.get_pane(&tile_id).cloned() else {
            return false;
        };

        *self.tab_close_request = Some((tile_id, book_id));

        false
    }

    fn retain_pane(&mut self, pane: &WorkspacePane) -> bool {
        match pane {
            WorkspacePane::Gallery => true,
            WorkspacePane::Book(book_id) => self
                .scene
                .open_books
                .iter()
                .any(|open_book| open_book.book_id.as_str() == book_id.as_str()),
        }
    }

    fn is_tile_draggable(&self, _tiles: &Tiles<WorkspacePane>, _tile_id: TileId) -> bool {
        false
    }

    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions {
            prune_single_child_tabs: false,
            ..Default::default()
        }
    }
}
