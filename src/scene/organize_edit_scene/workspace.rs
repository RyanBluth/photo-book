use std::{collections::HashMap, path::PathBuf};

use egui::{Id, Response, Sense, Stroke, TextStyle, Ui, Vec2, vec2};
use egui_tiles::{
    Behavior as TileBehavior, Container, SimplificationOptions, TabState, Tile, TileId, Tiles,
    Tree, UiResponse as TileUiResponse,
};

use crate::{
    cursor_manager::CursorManager,
    dep_mut,
    photo::Photo,
    project::{ProjectWorkspaceTab, ProjectWorkspaceTabs},
    scene::{Scene, viewer_scene::ViewerScene},
};

use super::{BookId, OrganizeEditScene, SceneResponse, photo_viewer::PhotoViewerId};

#[derive(Debug, Clone, PartialEq)]
pub(super) enum WorkspacePane {
    Gallery,
    Book(BookId),
    PhotoViewer(PhotoViewerId),
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
        self.insert_workspace_tab(WorkspacePane::Book(book_id))
    }

    pub(super) fn insert_photo_viewer_tab(&mut self, viewer_id: PhotoViewerId) -> TileId {
        self.insert_workspace_tab(WorkspacePane::PhotoViewer(viewer_id))
    }

    fn insert_workspace_tab(&mut self, pane: WorkspacePane) -> TileId {
        if self.workspace_tabs.root.is_none() {
            self.workspace_tabs = Self::initial_workspace_tabs();
        }

        let tile_id = self.workspace_tabs.tiles.insert_pane(pane);

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

    pub(super) fn activate_photo_viewer_tab(&mut self, viewer_id: &str) -> bool {
        self.workspace_tabs.make_active(|_, tile| {
            matches!(tile, Tile::Pane(WorkspacePane::PhotoViewer(active_viewer_id)) if active_viewer_id.as_str() == viewer_id)
        })
    }

    pub(super) fn close_workspace_tab(
        &mut self,
        workspace_tabs: &mut Tree<WorkspacePane>,
        tile_id: TileId,
        pane: WorkspacePane,
    ) {
        match pane {
            WorkspacePane::Gallery => {}
            WorkspacePane::Book(book_id) => {
                self.persist_book_state(&book_id);
                self.open_books
                    .retain(|open_book| open_book.tile_id != tile_id);
            }
            WorkspacePane::PhotoViewer(viewer_id) => {
                self.open_photo_viewers.retain(|open_viewer| {
                    open_viewer.tile_id != tile_id && open_viewer.viewer_id != viewer_id
                });
            }
        }

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

    fn project_workspace_tab_for_pane(&self, pane: &WorkspacePane) -> Option<ProjectWorkspaceTab> {
        match pane {
            WorkspacePane::Gallery => Some(ProjectWorkspaceTab::Gallery),
            WorkspacePane::Book(id) => Some(ProjectWorkspaceTab::Book { id: id.clone() }),
            WorkspacePane::PhotoViewer(viewer_id) => self
                .open_photo_viewers
                .iter()
                .find(|open_viewer| open_viewer.viewer_id.as_str() == viewer_id.as_str())
                .map(|open_viewer| ProjectWorkspaceTab::PhotoViewer {
                    path: open_viewer.scene.read().unwrap().photo_path().clone(),
                }),
        }
    }

    pub fn project_workspace_tabs(&self) -> ProjectWorkspaceTabs {
        let open_tabs = self
            .workspace_tabs
            .root
            .and_then(|root| self.workspace_tabs.tiles.get(root))
            .and_then(|tile| match tile {
                Tile::Container(container) => Some(container.children_vec()),
                Tile::Pane(_) => None,
            })
            .unwrap_or_default()
            .into_iter()
            .filter_map(|tile_id| match self.workspace_tabs.tiles.get(tile_id) {
                Some(Tile::Pane(pane)) => self.project_workspace_tab_for_pane(pane),
                _ => None,
            })
            .collect::<Vec<_>>();

        let selected_tab = self
            .active_workspace_pane()
            .and_then(|pane| self.project_workspace_tab_for_pane(&pane));

        ProjectWorkspaceTabs {
            open_tabs: if open_tabs.is_empty() {
                vec![ProjectWorkspaceTab::Gallery]
            } else {
                open_tabs
            },
            selected_tab,
        }
    }

    pub fn restore_workspace_tabs(
        &mut self,
        workspace_tabs: &ProjectWorkspaceTabs,
        photos_by_path: &HashMap<PathBuf, Photo>,
    ) {
        self.open_books.clear();
        self.open_photo_viewers.clear();
        self.workspace_tabs = Self::initial_workspace_tabs();
        self.selected_book_id = None;

        for tab in &workspace_tabs.open_tabs {
            match tab {
                ProjectWorkspaceTab::Gallery => {}
                ProjectWorkspaceTab::Book { id } => {
                    self.select_book(id);
                }
                ProjectWorkspaceTab::PhotoViewer { path } => {
                    if let Some(photo) = photos_by_path.get(path) {
                        self.open_photo_viewer(ViewerScene::new(photo.clone()));
                    }
                }
            }
        }

        match &workspace_tabs.selected_tab {
            Some(ProjectWorkspaceTab::Book { id }) if self.activate_book_tab(id) => {
                self.selected_book_id = Some(id.clone());
            }
            Some(ProjectWorkspaceTab::PhotoViewer { path }) => {
                let viewer_id = self
                    .open_photo_viewers
                    .iter()
                    .find(|open_viewer| open_viewer.scene.read().unwrap().photo_path() == path)
                    .map(|open_viewer| open_viewer.viewer_id.clone());

                if let Some(viewer_id) = viewer_id {
                    self.activate_photo_viewer_tab(&viewer_id);
                    self.selected_book_id = None;
                } else {
                    self.sync_active_workspace_tab();
                }
            }
            _ => {
                self.activate_gallery_tab();
                self.selected_book_id = None;
            }
        }
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
            Some(WorkspacePane::PhotoViewer(viewer_id)) => {
                if self
                    .open_photo_viewers
                    .iter()
                    .any(|open_viewer| open_viewer.viewer_id == viewer_id)
                {
                    if self.selected_book_id.is_some() {
                        self.persist_active_book_state();
                    }
                    self.selected_book_id = None;
                } else {
                    self.activate_gallery_tab();
                    self.selected_book_id = None;
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

        if let Some((tile_id, pane)) = tab_close_request {
            self.close_workspace_tab(&mut workspace_tabs, tile_id, pane);
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
    tab_close_request: &'a mut Option<(TileId, WorkspacePane)>,
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

    fn photo_viewer_name(&self, viewer_id: &str) -> String {
        self.scene
            .open_photo_viewers
            .iter()
            .find(|open_viewer| open_viewer.viewer_id.as_str() == viewer_id)
            .map(|open_viewer| {
                open_viewer
                    .scene
                    .read()
                    .unwrap()
                    .photo_file_name()
                    .to_string()
            })
            .unwrap_or_else(|| "Photo".to_string())
    }
}

impl TileBehavior<WorkspacePane> for WorkspaceTabsBehavior<'_> {
    fn pane_ui(
        &mut self,
        ui: &mut Ui,
        tile_id: TileId,
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
            WorkspacePane::PhotoViewer(viewer_id) => {
                let response = self
                    .scene
                    .open_photo_viewers
                    .iter_mut()
                    .find(|open_viewer| open_viewer.viewer_id.as_str() == viewer_id.as_str())
                    .map(|open_viewer| open_viewer.scene.write().unwrap().ui(ui))
                    .unwrap_or(SceneResponse::None);

                if matches!(response, SceneResponse::Pop(_)) {
                    *self.tab_close_request = Some((tile_id, pane.clone()));
                    *self.scene_response = Some(SceneResponse::None);
                } else {
                    *self.scene_response = Some(response);
                }
            }
        }

        TileUiResponse::None
    }

    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        32.0
    }

    fn tab_ui(
        &mut self,
        tiles: &mut Tiles<WorkspacePane>,
        ui: &mut Ui,
        id: Id,
        tile_id: TileId,
        state: &TabState,
    ) -> Response {
        let text = self.tab_title_for_tile(tiles, tile_id);
        let close_btn_size = Vec2::splat(self.close_button_outer_size());
        let close_btn_left_padding = 4.0;
        let font_id = TextStyle::Button.resolve(ui.style());
        let galley = text.into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, font_id);

        let x_margin = self.tab_title_spacing(ui.visuals());

        let button_width = galley.size().x
            + 2.0 * x_margin
            + f32::from(state.closable) * (close_btn_left_padding + close_btn_size.x * 2.0);
        let (_, tab_rect) = ui.allocate_space(vec2(button_width, ui.available_height()));

        let draggable = self.is_tile_draggable(tiles, tile_id);
        let sense = if draggable {
            Sense::click_and_drag()
        } else {
            Sense::click()
        };
        let tab_response = ui.interact(tab_rect, id, sense);
        let tab_response = if draggable {
            tab_response.on_hover_cursor(self.tab_hover_cursor_icon())
        } else {
            tab_response
        };

        let is_hovered = tab_response.contains_pointer();

        if is_hovered {
            dep_mut!(CursorManager, |cm| {
                cm.set_cursor(egui::CursorIcon::PointingHand);
            })
        }

        // Show a gap when dragged
        if ui.is_rect_visible(tab_rect) && !state.is_being_dragged {
            let bg_color = self.tab_bg_color(ui.visuals(), tiles, tile_id, state);
            let stroke = self.tab_outline_stroke(ui.visuals(), tiles, tile_id, state);
            ui.painter().rect(
                tab_rect.shrink(0.5),
                0.0,
                bg_color,
                stroke,
                egui::StrokeKind::Inside,
            );

            if state.active {
                // Make the tab name area connect with the tab ui area:
                ui.painter().hline(
                    tab_rect.x_range().shrink(stroke.width),
                    tab_rect.bottom(),
                    Stroke::new(stroke.width + 2.0, bg_color),
                );
            }

            // Prepare title's text for rendering
            let text_color = self.tab_text_color(ui.visuals(), tiles, tile_id, state);
            let text_rect = tab_rect.shrink(x_margin).translate(if state.closable {
                Vec2::new(close_btn_size.x, 0.0)
            } else {
                Vec2::ZERO
            });
            let text_position = egui::Align2::LEFT_CENTER
                .align_size_within_rect(galley.size(), text_rect)
                .min;

            // Render the title
            ui.painter().galley(text_position, galley, text_color);

            // Conditionally render the close button
            if state.closable && is_hovered {
                let close_btn_rect = egui::Align2::RIGHT_CENTER
                    .align_size_within_rect(close_btn_size, tab_rect.shrink(x_margin));

                // Allocate
                let close_btn_id = ui.auto_id_with("tab_close_btn");
                let close_btn_response = ui
                    .interact(close_btn_rect, close_btn_id, Sense::click_and_drag())
                    .on_hover_cursor(egui::CursorIcon::Default);

                let visuals = ui.style().interact(&close_btn_response);

                // Scale based on the interaction visuals
                let rect = close_btn_rect.shrink(2.0);

                let stroke = Stroke::new(2.5, visuals.fg_stroke.color);

                // paint the crossed lines
                ui.painter() // paints \
                    .line_segment([rect.left_top(), rect.right_bottom()], stroke);
                ui.painter() // paints /
                    .line_segment([rect.right_top(), rect.left_bottom()], stroke);

                // Give the user a chance to react to the close button being clicked
                // Only close if the user returns true (handled)
                if close_btn_response.clicked()
                    || tab_response.clicked_by(egui::PointerButton::Middle)
                {
                    log::debug!("Tab close requested for tile: {tile_id:?}");

                    // Close the tab if the implementation wants to
                    if self.on_tab_close(tiles, tile_id) {
                        log::debug!("Implementation confirmed close request for tile: {tile_id:?}");

                        tiles.remove(tile_id);
                    } else {
                        log::debug!("Implementation denied close request for tile: {tile_id:?}");
                    }
                }
            }
        }

        self.on_tab_button(tiles, tile_id, tab_response)
    }

    fn tab_title_for_pane(&mut self, pane: &WorkspacePane) -> egui::WidgetText {
        match pane {
            WorkspacePane::Gallery => "Gallery".into(),
            WorkspacePane::Book(book_id) => self.book_name(book_id).into(),
            WorkspacePane::PhotoViewer(viewer_id) => self.photo_viewer_name(viewer_id).into(),
        }
    }

    fn is_tab_closable(&self, tiles: &Tiles<WorkspacePane>, tile_id: TileId) -> bool {
        matches!(
            tiles.get_pane(&tile_id),
            Some(WorkspacePane::Book(_) | WorkspacePane::PhotoViewer(_))
        )
    }

    fn on_tab_close(&mut self, tiles: &mut Tiles<WorkspacePane>, tile_id: TileId) -> bool {
        let Some(pane @ (WorkspacePane::Book(_) | WorkspacePane::PhotoViewer(_))) =
            tiles.get_pane(&tile_id).cloned()
        else {
            return false;
        };

        *self.tab_close_request = Some((tile_id, pane));

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
            WorkspacePane::PhotoViewer(viewer_id) => self
                .scene
                .open_photo_viewers
                .iter()
                .any(|open_viewer| open_viewer.viewer_id.as_str() == viewer_id.as_str()),
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
