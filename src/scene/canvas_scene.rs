use std::fmt::Display;

use egui::{Id, Key, Rect, Ui, Vec2};
use egui_tiles::{Container, ContainerKind, SimplificationOptions, Tile, TileId, UiResponse};
use indexmap::{IndexMap, indexmap};

use crate::{
    dep, dep_mut,
    export::{ExportTaskId, ExportTaskStatus, Exporter},
    history::{HistoricallyEqual, UndoRedoStack},
    id::{LayerId, PageId, next_layer_id, next_page_id},
    model::edit_state::EditablePage,
    scene::crop_scene::CropScene,
    theme::color,
    utils::{IdExt, RectExt},
    widget::{
        canvas::{Canvas, CanvasState, MultiSelect},
        canvas_info::{
            layers::{Layer, LayerContent},
            panel::{CanvasAdjustments, CanvasArrange, CanvasLayers, CanvasProperties},
            quick_layout::{QuickLayout, QuickLayoutState},
        },
        image_gallery::{ImageGallery, ImageGalleryState},
        pages::{Pages, PagesResponse, PagesState},
        photo_adjustments::PhotoAdjustmentsState,
        templates::{Templates, TemplatesResponse, TemplatesState},
    },
};

use super::{
    NavigationRequest, Navigator, Scene, ScenePopResponse, SceneResponse, SceneTransition::Viewer,
    crop_scene::CropSceneResponse, viewer_scene::ViewerScene,
};

use crate::widget::canvas::CanvasResponse;

#[derive(Debug, Clone)]
pub struct CanvasSceneState {
    pub gallery_state: ImageGalleryState,
    pub pages_state: PagesState,
    history_managers: IndexMap<PageId, CanvasHistoryManager>,
    templates_state: TemplatesState,
    export_task_id: Option<ExportTaskId>,
    quick_layout_state: QuickLayoutState,
    adjustments_state: PhotoAdjustmentsState,
    adjustments_history_pending: Option<PageId>,
    pub clipboard: Option<Vec<Layer>>,
}

impl CanvasSceneState {
    pub fn new() -> Self {
        let page_id = next_page_id();
        let initial_state = CanvasState::new();

        Self {
            gallery_state: ImageGalleryState::default(),
            history_managers: indexmap! {
                page_id => CanvasHistoryManager::with_initial_state(initial_state.clone())
            },
            pages_state: PagesState::new(indexmap! { page_id => initial_state }, page_id),
            templates_state: TemplatesState::new(),
            export_task_id: None,
            quick_layout_state: QuickLayoutState::new(),
            adjustments_state: PhotoAdjustmentsState::new(),
            adjustments_history_pending: None,
            clipboard: None,
        }
    }

    pub fn with_pages(pages: IndexMap<PageId, CanvasState>, selected_page: PageId) -> Self {
        let history_managers = pages
            .iter()
            .map(|(page_id, page)| {
                (
                    *page_id,
                    CanvasHistoryManager::with_initial_state(page.clone()),
                )
            })
            .collect();

        Self {
            gallery_state: ImageGalleryState::default(),
            history_managers,
            pages_state: PagesState::new(pages, selected_page),
            templates_state: TemplatesState::new(),
            export_task_id: None,
            quick_layout_state: QuickLayoutState::new(),
            adjustments_state: PhotoAdjustmentsState::new(),
            adjustments_history_pending: None,
            clipboard: None,
        }
    }

    pub fn selected_page_mut(&mut self) -> &mut CanvasState {
        self.pages_state
            .pages
            .get_mut(&self.pages_state.selected_page)
            .unwrap()
    }

    pub fn selected_page(&self) -> &CanvasState {
        self.pages_state
            .pages
            .get(&self.pages_state.selected_page)
            .unwrap()
    }

    pub fn selected_page_and_history_mut(
        &mut self,
    ) -> (&mut CanvasState, &mut CanvasHistoryManager) {
        let selected_page = self.pages_state.selected_page;
        let page = self.pages_state.pages.get_mut(&selected_page).unwrap();
        let history = self.history_managers.get_mut(&selected_page).unwrap();
        (page, history)
    }

    pub fn has_pages(&self) -> bool {
        !self.pages_state.pages.is_empty()
    }

    fn sync_history_managers(&mut self) {
        self.history_managers
            .retain(|page_id, _| self.pages_state.pages.contains_key(page_id));
        for (page_id, page) in &self.pages_state.pages {
            self.history_managers
                .entry(*page_id)
                .or_insert_with(|| CanvasHistoryManager::with_initial_state(page.clone()));
        }
    }

    fn save_history_for_page(&mut self, page_id: PageId, kind: CanvasHistoryKind) {
        let Some(page_snapshot) = self.pages_state.pages.get(&page_id).cloned() else {
            return;
        };
        if let Some(history) = self.history_managers.get_mut(&page_id) {
            history.save_history(kind, &page_snapshot);
        }
    }

    fn save_selected_history(&mut self, kind: CanvasHistoryKind) {
        self.save_history_for_page(self.pages_state.selected_page, kind);
    }

    fn flush_pending_adjustment_history(&mut self, pointer_down: bool) {
        if pointer_down {
            return;
        }
        if let Some(page_id) = self.adjustments_history_pending.take() {
            self.save_history_for_page(page_id, CanvasHistoryKind::AdjustPhoto);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanvasScenePane {
    Gallery,
    Canvas,
    Arrange,
    Properties,
    Adjustments,
    Layers,
    Pages,
    Templates,
    QuickLayout,
}

#[derive(Debug, Clone)]
pub struct CanvasScene {
    pub state: CanvasSceneState,
    pub tree: egui_tiles::Tree<CanvasScenePane>,
}

impl CanvasScene {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::with_state_and_tree_id(CanvasSceneState::new(), "canvas_scene_tree")
    }

    pub fn with_state_and_tree_id(state: CanvasSceneState, tree_id: impl Into<Id>) -> Self {
        let mut tiles = egui_tiles::Tiles::default();

        let left_tabs = vec![
            tiles.insert_pane(CanvasScenePane::Gallery),
            tiles.insert_pane(CanvasScenePane::Pages),
            tiles.insert_pane(CanvasScenePane::Templates),
        ];

        let left_tabs_ids = tiles.insert_tab_tile(left_tabs);
        let canvas_id = tiles.insert_pane(CanvasScenePane::Canvas);

        let properties_id = tiles.insert_pane(CanvasScenePane::Properties);
        let arrange_id = tiles.insert_pane(CanvasScenePane::Arrange);
        let adjustments_id = tiles.insert_pane(CanvasScenePane::Adjustments);
        let inspector_tabs_id =
            tiles.insert_tab_tile(vec![properties_id, arrange_id, adjustments_id]);

        let layers_id = tiles.insert_pane(CanvasScenePane::Layers);
        let quick_layout_id = tiles.insert_pane(CanvasScenePane::QuickLayout);
        let layers_tabs_id = tiles.insert_tab_tile(vec![layers_id, quick_layout_id]);
        let mut right_split = egui_tiles::Linear::new(
            egui_tiles::LinearDir::Vertical,
            vec![inspector_tabs_id, layers_tabs_id],
        );
        right_split.shares.set_share(inspector_tabs_id, 0.6);
        right_split.shares.set_share(layers_tabs_id, 0.4);

        // History is intentionally omitted from the default layout for now.
        let right_panel_id = tiles.insert_container(right_split);

        let children = vec![left_tabs_ids, canvas_id, right_panel_id];

        let mut linear_layout =
            egui_tiles::Linear::new(egui_tiles::LinearDir::Horizontal, children);
        linear_layout.shares.set_share(left_tabs_ids, 0.2);
        linear_layout.shares.set_share(right_panel_id, 0.2);

        Self {
            state,
            tree: egui_tiles::Tree::new(tree_id, tiles.insert_container(linear_layout), tiles),
        }
    }

    pub fn set_right_sidebar_open(&mut self, open: bool) {
        self.set_panes_visible(
            &[
                CanvasScenePane::Arrange,
                CanvasScenePane::Properties,
                CanvasScenePane::Adjustments,
                CanvasScenePane::Layers,
                CanvasScenePane::QuickLayout,
            ],
            open,
        );
    }

    pub fn set_left_sidebar_open(&mut self, open: bool) {
        self.set_panes_visible(
            &[
                CanvasScenePane::Gallery,
                CanvasScenePane::Pages,
                CanvasScenePane::Templates,
            ],
            open,
        );
    }

    fn set_panes_visible(&mut self, panes: &[CanvasScenePane], visible: bool) {
        if let Some(root) = self.tree.root {
            Self::set_matching_subtrees_visible(&mut self.tree.tiles, root, panes, visible);
        }
    }

    fn set_matching_subtrees_visible(
        tiles: &mut egui_tiles::Tiles<CanvasScenePane>,
        tile_id: TileId,
        panes: &[CanvasScenePane],
        visible: bool,
    ) -> PaneMembership {
        let membership = match tiles.get(tile_id) {
            Some(Tile::Pane(pane)) => PaneMembership {
                matches: panes.contains(pane),
                has_other_panes: !panes.contains(pane),
            },
            Some(Tile::Container(container)) => container
                .children()
                .copied()
                .map(|child| Self::pane_membership(tiles, child, panes))
                .fold(PaneMembership::default(), PaneMembership::merge),
            None => PaneMembership::default(),
        };

        if membership.matches && !membership.has_other_panes {
            tiles.set_visible(tile_id, visible);
        } else if membership.matches {
            let children = tiles
                .get_container(tile_id)
                .map(Container::children_vec)
                .unwrap_or_default();
            for child in children {
                Self::set_matching_subtrees_visible(tiles, child, panes, visible);
            }
        }

        membership
    }

    fn pane_membership(
        tiles: &egui_tiles::Tiles<CanvasScenePane>,
        tile_id: TileId,
        panes: &[CanvasScenePane],
    ) -> PaneMembership {
        match tiles.get(tile_id) {
            Some(Tile::Pane(pane)) => PaneMembership {
                matches: panes.contains(pane),
                has_other_panes: !panes.contains(pane),
            },
            Some(Tile::Container(container)) => container
                .children()
                .copied()
                .map(|child| Self::pane_membership(tiles, child, panes))
                .fold(PaneMembership::default(), PaneMembership::merge),
            None => PaneMembership::default(),
        }
    }

    fn ensure_non_canvas_panes_have_tabs(&mut self) {
        let tile_ids = self.tree.tiles.tile_ids().collect::<Vec<_>>();

        for tile_id in tile_ids {
            if !matches!(
                self.tree.tiles.get_pane(&tile_id),
                Some(pane) if pane != &CanvasScenePane::Canvas
            ) {
                continue;
            }

            let parent_is_tabs = self
                .tree
                .tiles
                .parent_of(tile_id)
                .and_then(|parent_id| self.tree.tiles.get(parent_id))
                .and_then(Tile::container_kind)
                == Some(ContainerKind::Tabs);

            if parent_is_tabs {
                continue;
            }

            let Some(tile) = self.tree.tiles.remove(tile_id) else {
                continue;
            };
            let new_tile_id = self.tree.tiles.next_free_id();
            self.tree.tiles.insert(new_tile_id, tile);
            self.tree.tiles.insert(
                tile_id,
                Tile::Container(Container::new_tabs(vec![new_tile_id])),
            );
        }
    }

    #[allow(dead_code)]
    pub fn with_state(state: CanvasSceneState) -> Self {
        Self::with_state_and_tree_id(state, "canvas_scene_tree")
    }
}

#[derive(Clone, Copy, Default)]
struct PaneMembership {
    matches: bool,
    has_other_panes: bool,
}

impl PaneMembership {
    fn merge(self, other: Self) -> Self {
        Self {
            matches: self.matches || other.matches,
            has_other_panes: self.has_other_panes || other.has_other_panes,
        }
    }
}

impl Scene for CanvasScene {
    fn ui(&mut self, ui: &mut egui::Ui) -> SceneResponse {
        // Remove the sync code since we're working directly with the selected page

        match self.state.export_task_id {
            Some(task_id) => {
                let status = dep!(Exporter, |exporter| exporter.get_task_status(task_id));

                match status {
                    Some(ExportTaskStatus::Failed(error)) => {
                        log::error!("Export failed: {:?}", error);
                        self.state.export_task_id = None;
                    }
                    Some(ExportTaskStatus::InProgress(progress)) => {
                        log::info!("Exporting... {:.0}%", progress * 100.0);
                    }
                    Some(ExportTaskStatus::Completed) | None => {
                        log::info!("Export Complete");
                        self.state.export_task_id = None;
                    }
                    Some(ExportTaskStatus::Cancelled) => {
                        log::info!("Export cancelled");
                        self.state.export_task_id = None;
                    }
                }
            }
            None => {
                if ui.ctx().input(|input| input.key_pressed(Key::F1)) {
                    self.state.export_task_id = Some(dep_mut!(Exporter, |exporter| {
                        exporter.export(
                            ui.ctx().clone(),
                            self.state.pages_state.pages.values().cloned().collect(),
                            "export".into(),
                            "out",
                        )
                    }));
                }
            }
        }

        let mut navigator = Navigator::new();

        self.state.sync_history_managers();
        self.ensure_non_canvas_panes_have_tabs();

        self.tree.ui(
            &mut ViewerTreeBehavior {
                scene_state: &mut self.state,
                navigator: &mut navigator,
            },
            ui,
        );
        self.state
            .flush_pending_adjustment_history(ui.input(|input| input.pointer.primary_down()));

        match navigator.process_pending_request() {
            Some(NavigationRequest::Push(scene_state)) => SceneResponse::Push(scene_state),
            Some(NavigationRequest::Pop(response)) => SceneResponse::Pop(response),
            None => SceneResponse::None,
        }
    }

    fn popped(&mut self, popped_scene_response: ScenePopResponse) {
        if let ScenePopResponse::Crop(CropSceneResponse::Apply {
            layer_id,
            page_id,
            crop,
        }) = popped_scene_response
        {
            let page = self.state.pages_state.pages.get_mut(&page_id).unwrap();
            let Some(layer) = page.editable_layer_mut(&layer_id) else {
                return;
            };
            if let LayerContent::Photo(photo) = &mut layer.content {
                photo.crop = crop;

                let photo_rect: Rect = Rect::from_center_size(
                    layer.transform_state.rect.center(),
                    Vec2::new(
                        photo.photo.metadata.rotated_width() as f32 * crop.size().x,
                        photo.photo.metadata.rotated_height() as f32 * crop.size().y,
                    ),
                );

                layer.transform_state.rect =
                    photo_rect.fit_and_center_within(layer.transform_state.rect);
            }
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
        match pane {
            CanvasScenePane::Gallery => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, color::SIDE_PANEL_BACKGROUND);
                let response = ImageGallery::show(ui, &mut self.scene_state.gallery_state, None);

                // Handle primary action (double-click)
                if let Some(photo) = response.primary_action_photo {
                    let is_template = self.scene_state.selected_page().template.is_some();

                    if is_template {
                        if self
                            .scene_state
                            .selected_page_mut()
                            .replace_selected_template_photo(photo.clone())
                        {
                            // Create a snapshot of the state after modification
                            self.scene_state
                                .save_selected_history(CanvasHistoryKind::AddPhoto);
                        }
                    } else {
                        self.scene_state
                            .selected_page_mut()
                            .add_photo(photo.clone());
                        // Create a snapshot of the state after modification
                        self.scene_state
                            .save_selected_history(CanvasHistoryKind::AddPhoto);
                    }
                }

                // Handle secondary action (right-click)
                if let Some(photo) = response.secondary_action_photo {
                    self.navigator.push(Viewer(ViewerScene::new(photo.clone())));
                }
            }
            CanvasScenePane::Canvas => {
                if !self.scene_state.has_pages() {
                    ui.centered_and_justified(|ui| {
                        ui.heading("Add a page to get started");
                    });
                    return UiResponse::None;
                }

                self.handle_keys(ui);

                let rect = ui.max_rect();
                let canvas_response = {
                    let (page, history) = self.scene_state.selected_page_and_history_mut();
                    Canvas::new(page, rect, history).show(ui)
                };

                match canvas_response {
                    Some(CanvasResponse::EnterCropMode { target_layer }) => {
                        let page_id = self.scene_state.pages_state.selected_page;
                        let Some(photo) = self
                            .scene_state
                            .selected_page()
                            .editable_layer(&target_layer)
                            .and_then(|layer| match &layer.content {
                                LayerContent::Photo(photo) => Some(photo.clone()),
                                _ => None,
                            })
                        else {
                            return UiResponse::None;
                        };
                        let crop_scene = CropScene::new(
                            target_layer,
                            page_id,
                            rect,
                            photo.photo,
                            photo.adjustments,
                            photo.crop,
                        );
                        self.navigator
                            .push(super::SceneTransition::Crop(crop_scene));
                    }
                    Some(CanvasResponse::Exit) => {
                        return UiResponse::None;
                    }
                    None => {}
                }
            }
            CanvasScenePane::Arrange => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, color::SIDE_PANEL_BACKGROUND);

                if !self.scene_state.has_pages() {
                    ui.centered_and_justified(|ui| {
                        ui.heading("No page selected");
                    });
                    return UiResponse::None;
                }

                CanvasArrange {
                    canvas_state: self.scene_state.selected_page_mut(),
                }
                .show(ui);
            }
            CanvasScenePane::Properties => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, color::SIDE_PANEL_BACKGROUND);

                if !self.scene_state.has_pages() {
                    ui.centered_and_justified(|ui| {
                        ui.heading("No page selected");
                    });
                    return UiResponse::None;
                }

                CanvasProperties {
                    canvas_state: self.scene_state.selected_page_mut(),
                }
                .show(ui);
            }
            CanvasScenePane::Adjustments => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, color::SIDE_PANEL_BACKGROUND);

                if !self.scene_state.has_pages() {
                    ui.centered_and_justified(|ui| {
                        ui.heading("No page selected");
                    });
                    return UiResponse::None;
                }

                let response = {
                    let pages_state = &mut self.scene_state.pages_state;
                    let canvas_state = pages_state
                        .pages
                        .get_mut(&pages_state.selected_page)
                        .unwrap();

                    CanvasAdjustments {
                        canvas_state,
                        adjustments_state: &mut self.scene_state.adjustments_state,
                    }
                    .show(ui)
                };

                if response {
                    self.scene_state.adjustments_history_pending =
                        Some(self.scene_state.pages_state.selected_page);
                }
            }
            CanvasScenePane::Layers => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, color::SIDE_PANEL_BACKGROUND);

                if !self.scene_state.has_pages() {
                    ui.centered_and_justified(|ui| {
                        ui.heading("No page selected");
                    });
                    return UiResponse::None;
                }

                let response = CanvasLayers {
                    canvas_state: self.scene_state.selected_page_mut(),
                }
                .show(ui);

                if let Some(history_kind) = response.history {
                    self.scene_state.save_selected_history(history_kind);
                }
            }
            CanvasScenePane::Pages => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, color::SIDE_PANEL_BACKGROUND);

                match Pages::new(&mut self.scene_state.pages_state).show(ui) {
                    PagesResponse::SelectPage => {
                        // No need to sync canvas_state anymore
                    }
                    PagesResponse::None => {}
                }
            }
            CanvasScenePane::Templates => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, color::SIDE_PANEL_BACKGROUND);

                match Templates::new(&mut self.scene_state.templates_state).show(ui) {
                    TemplatesResponse::SelectTemplate(template) => {
                        let new_page_id = next_page_id();
                        let new_canvas_state = CanvasState::with_template(template.clone());

                        self.scene_state
                            .pages_state
                            .pages
                            .insert(new_page_id, new_canvas_state);

                        self.scene_state.pages_state.selected_page = new_page_id;
                    }
                    TemplatesResponse::None => {}
                }
            }
            CanvasScenePane::QuickLayout => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, color::SIDE_PANEL_BACKGROUND);

                if !self.scene_state.has_pages() {
                    ui.centered_and_justified(|ui| {
                        ui.heading("No page selected");
                    });
                    return UiResponse::None;
                }

                let mut quick_layout_state = self.scene_state.quick_layout_state.clone();
                let (page, history) = self.scene_state.selected_page_and_history_mut();
                QuickLayout::new(&mut quick_layout_state, page, history).show(ui);
                self.scene_state.quick_layout_state = quick_layout_state;
            }
        }

        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &CanvasScenePane) -> egui::widget_text::WidgetText {
        match pane {
            CanvasScenePane::Gallery => "Gallery".into(),
            CanvasScenePane::Canvas => "Canvas".into(),
            CanvasScenePane::Arrange => "Arrange".into(),
            CanvasScenePane::Properties => "Properties".into(),
            CanvasScenePane::Adjustments => "Adjustments".into(),
            CanvasScenePane::Layers => "Layers".into(),
            CanvasScenePane::Pages => "Pages".into(),
            CanvasScenePane::Templates => "Templates".into(),
            CanvasScenePane::QuickLayout => "Quick Layout".into(),
        }
    }

    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions {
            prune_single_child_tabs: false,
            ..Default::default()
        }
    }
}

impl<'a> ViewerTreeBehavior<'a> {
    fn handle_keys(&mut self, ui: &mut Ui) {
        if ui.ctx().text_edit_focused() {
            return;
        }

        let copy = ui.input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, Key::C));
        let paste = ui.input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, Key::V));

        if copy {
            let selected_page_id = self.scene_state.pages_state.selected_page;
            let selected_layers = self
                .scene_state
                .pages_state
                .pages
                .get(&selected_page_id)
                .unwrap()
                .layers
                .values()
                .filter(|layer| layer.selected)
                .cloned()
                .collect::<Vec<Layer>>();

            if !selected_layers.is_empty() {
                self.scene_state.clipboard = Some(selected_layers);
            }
        }

        if paste {
            let clipboard_content = self.scene_state.clipboard.clone();

            if let Some(clipboard_layers) = clipboard_content {
                let offset = Vec2::new(20.0, 20.0);
                let page_id = self.scene_state.pages_state.selected_page;
                let page = self
                    .scene_state
                    .pages_state
                    .pages
                    .get_mut(&page_id)
                    .unwrap();

                for layer in clipboard_layers {
                    let mut new_layer = layer.clone();
                    new_layer.id = next_layer_id();
                    new_layer.selected = true;
                    new_layer.transform_state.id = Id::random();

                    // Offset the pasted layer slightly so it's visible
                    new_layer.transform_state.rect =
                        new_layer.transform_state.rect.translate(offset);

                    for layer in page.layers.values_mut() {
                        layer.selected = false;
                    }

                    page.layers.insert(new_layer.id, new_layer.clone());
                    page.quick_layout_order.push(new_layer.id);
                }

                page.update_quick_layout_order();

                self.scene_state
                    .save_history_for_page(page_id, CanvasHistoryKind::AddPhoto);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanvasHistoryKind {
    Transform,
    AddPhoto,
    DeletePhoto,
    _Select,
    _Page, // TODO Add specific cases for things within the page settings
    AddText,
    EditText,
    SelectLayer,
    DeselectLayer,
    AdjustPhoto,
    QuickLayout,
    AddShape,
    EditLayer,
}

impl Display for CanvasHistoryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CanvasHistoryKind::Transform => write!(f, "Move"),
            CanvasHistoryKind::AddPhoto => write!(f, "Add Photo"),
            CanvasHistoryKind::DeletePhoto => write!(f, "Delete Photo"),
            CanvasHistoryKind::_Select => write!(f, "Select"),
            CanvasHistoryKind::_Page => write!(f, "Page"),
            CanvasHistoryKind::AddText => write!(f, "Add Text"),
            CanvasHistoryKind::EditText => write!(f, "Edit Text"),
            CanvasHistoryKind::SelectLayer => write!(f, "Select Layer"),
            CanvasHistoryKind::DeselectLayer => write!(f, "Deselect Layer"),
            CanvasHistoryKind::AdjustPhoto => write!(f, "Adjust Photo"),
            CanvasHistoryKind::QuickLayout => write!(f, "Quick Layout"),
            CanvasHistoryKind::AddShape => write!(f, "Add Shape"),
            CanvasHistoryKind::EditLayer => write!(f, "Edit Layer"),
        }
    }
}

impl HistoricallyEqual for CanvasHistory {
    fn historically_equal_to(&self, other: &Self) -> bool {
        self.layers.len() == other.layers.len()
            && self
                .layers
                .values()
                .zip(other.layers.values())
                .all(|(a, b)| a.historically_equal_to(b))
            && self.page == other.page
            && self.multi_select == other.multi_select
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasHistory {
    layers: IndexMap<LayerId, Layer>,
    multi_select: Option<MultiSelect>,
    page: EditablePage,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasHistoryManager {
    pub stack: UndoRedoStack<CanvasHistoryKind, CanvasHistory>,
}

impl CanvasHistoryManager {
    pub fn preview() -> Self {
        Self::with_initial_state(CanvasState::new())
    }

    pub fn with_initial_state(state: CanvasState) -> Self {
        CanvasHistoryManager {
            stack: UndoRedoStack::new(CanvasHistory {
                layers: state.layers.clone(),
                multi_select: state.multi_select.clone(),
                page: state.page.clone(),
            }),
        }
    }

    pub fn _is_at_end(&self) -> bool {
        self.stack.index == self.stack.history.len()
    }

    pub fn undo(&mut self, canvas_state: &mut CanvasState) {
        let new_value = self.stack.undo();
        self.apply_history(new_value, canvas_state);
    }

    pub fn redo(&mut self, canvas_state: &mut CanvasState) {
        let new_value = self.stack.redo();
        self.apply_history(new_value, canvas_state);
    }

    pub fn save_history(&mut self, kind: CanvasHistoryKind, canvas_state: &CanvasState) {
        self.stack.save_history(
            kind,
            CanvasHistory {
                layers: canvas_state.layers.clone(),
                multi_select: canvas_state.multi_select.clone(),
                page: canvas_state.page.clone(),
            },
        );
    }

    fn apply_history(&mut self, history: CanvasHistory, canvas_state: &mut CanvasState) {
        canvas_state.layers = history.layers;
        canvas_state.multi_select = history.multi_select;
        canvas_state.page = history.page;
    }

    pub fn _apply_index(&mut self, index: usize, canvas_state: &mut CanvasState) {
        let history = &self.stack.history[index];
        self.apply_history(history.1.clone(), canvas_state);
    }

    pub fn _capturing_history<T>(
        &mut self,
        kind: CanvasHistoryKind,
        canvas_state: &mut CanvasState,
        perform: impl FnOnce(&mut CanvasState) -> T,
    ) -> T {
        let mut state_clone = canvas_state.clone();
        let res: T = perform(&mut state_clone);
        let changed = state_clone != *canvas_state;
        *canvas_state = state_clone;
        if changed {
            self.save_history(kind, canvas_state);
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::page::Page;

    #[test]
    fn history_is_isolated_per_page() {
        let first_page_id = next_page_id();
        let second_page_id = next_page_id();
        let mut state = CanvasSceneState::with_pages(
            indexmap! {
                first_page_id => CanvasState::new(),
                second_page_id => CanvasState::new(),
            },
            first_page_id,
        );

        state
            .pages_state
            .pages
            .get_mut(&first_page_id)
            .unwrap()
            .page = EditablePage::new(Page::with_size_inches(5.0, 7.0));
        state.save_history_for_page(first_page_id, CanvasHistoryKind::_Page);
        state
            .pages_state
            .pages
            .get_mut(&first_page_id)
            .unwrap()
            .page = EditablePage::new(Page::with_size_inches(6.0, 8.0));
        state.save_history_for_page(first_page_id, CanvasHistoryKind::_Page);
        state
            .pages_state
            .pages
            .get_mut(&second_page_id)
            .unwrap()
            .page = EditablePage::new(Page::with_size_inches(10.0, 12.0));
        state.save_history_for_page(second_page_id, CanvasHistoryKind::_Page);

        assert_eq!(
            state.history_managers[&first_page_id].stack.history.len(),
            2
        );
        assert_eq!(
            state.history_managers[&second_page_id].stack.history.len(),
            1
        );

        let first_page = state.pages_state.pages.get_mut(&first_page_id).unwrap();
        state
            .history_managers
            .get_mut(&first_page_id)
            .unwrap()
            .undo(first_page);

        assert_eq!(state.pages_state.pages[&first_page_id].page.size().x, 5.0);
        assert_eq!(state.pages_state.pages[&second_page_id].page.size().x, 10.0);
        assert_eq!(state.history_managers[&second_page_id].stack.index, 0);
    }

    #[test]
    fn sidebar_toggle_collapses_default_subtree() {
        let mut scene = CanvasScene::new();
        let properties = scene
            .tree
            .tiles
            .find_pane(&CanvasScenePane::Properties)
            .unwrap();
        let right_subtree = scene
            .tree
            .tiles
            .parent_of(properties)
            .and_then(|tabs| scene.tree.tiles.parent_of(tabs))
            .unwrap();

        scene.set_right_sidebar_open(false);
        assert!(!scene.tree.tiles.is_visible(right_subtree));
        assert!(
            scene.tree.tiles.is_visible(
                scene
                    .tree
                    .tiles
                    .find_pane(&CanvasScenePane::Canvas)
                    .unwrap()
            )
        );

        scene.set_right_sidebar_open(true);
        assert!(scene.tree.tiles.is_visible(right_subtree));
    }

    #[test]
    fn sidebar_toggle_handles_panes_mixed_with_canvas() {
        let mut tiles = egui_tiles::Tiles::default();
        let canvas = tiles.insert_pane(CanvasScenePane::Canvas);
        let arrange = tiles.insert_pane(CanvasScenePane::Arrange);
        let mixed_tabs = tiles.insert_tab_tile(vec![canvas, arrange]);
        let properties = tiles.insert_pane(CanvasScenePane::Properties);
        let root = tiles.insert_horizontal_tile(vec![mixed_tabs, properties]);
        let mut scene = CanvasScene {
            state: CanvasSceneState::new(),
            tree: egui_tiles::Tree::new("mixed_sidebar_tree", root, tiles),
        };

        scene.set_right_sidebar_open(false);
        assert!(scene.tree.tiles.is_visible(canvas));
        assert!(!scene.tree.tiles.is_visible(arrange));
        assert!(!scene.tree.tiles.is_visible(properties));

        scene.set_right_sidebar_open(true);
        assert!(scene.tree.tiles.is_visible(arrange));
        assert!(scene.tree.tiles.is_visible(properties));
    }
}
