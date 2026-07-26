use egui::{Id, Key, Ui, Vec2};
use egui_tiles::{Container, ContainerKind, SimplificationOptions, Tile, TileId, UiResponse};
use indexmap::{IndexMap, indexmap};

use crate::{
    dep, dep_mut,
    export::{ExportTaskId, ExportTaskStatus, Exporter},
    id::{PageId, next_page_id},
    scene::crop_scene::CropScene,
    theme::color,
    widget::{
        canvas::{Canvas, CanvasHistoryKind, CanvasHistoryManager, CanvasState},
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

    fn apply_page_edit<T>(
        &mut self,
        page_id: PageId,
        kind: CanvasHistoryKind,
        edit: impl FnOnce(&mut CanvasState) -> T,
    ) -> Option<T> {
        let page = self.pages_state.pages.get_mut(&page_id)?;
        let history = self.history_managers.get_mut(&page_id)?;
        Some(history.apply(kind, page, edit))
    }

    fn apply_selected_edit<T>(
        &mut self,
        kind: CanvasHistoryKind,
        edit: impl FnOnce(&mut CanvasState) -> T,
    ) -> Option<T> {
        self.apply_page_edit(self.pages_state.selected_page, kind, edit)
    }

    fn select_page(&mut self, page_id: PageId) {
        if page_id == self.pages_state.selected_page
            || !self.pages_state.pages.contains_key(&page_id)
        {
            return;
        }

        self.finish_selected_history();
        self.pages_state.selected_page = page_id;
    }

    fn finish_selected_history(&mut self) {
        if let Some(history) = self
            .history_managers
            .get_mut(&self.pages_state.selected_page)
        {
            history.finish_pending();
        }
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
        self.finish_inactive_history_scopes();
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

    fn active_history_kinds(tree: &egui_tiles::Tree<CanvasScenePane>) -> Vec<CanvasHistoryKind> {
        tree.active_tiles()
            .into_iter()
            .filter_map(|tile_id| tree.tiles.get_pane(&tile_id))
            .flat_map(CanvasScenePane::history_kinds)
            .copied()
            .collect()
    }

    fn finish_inactive_history_scopes(&mut self) {
        let active_history_kinds = Self::active_history_kinds(&self.tree);
        let selected_page = self.state.pages_state.selected_page;
        if let Some(history) = self.state.history_managers.get_mut(&selected_page) {
            for kind in CanvasScenePane::EDITOR_PANES
                .iter()
                .flat_map(CanvasScenePane::history_kinds)
                .filter(|kind| !active_history_kinds.contains(kind))
            {
                history.finish(*kind);
            }
        }
    }
}

impl CanvasScenePane {
    const EDITOR_PANES: [Self; 6] = [
        Self::Canvas,
        Self::Arrange,
        Self::Properties,
        Self::Adjustments,
        Self::Layers,
        Self::QuickLayout,
    ];

    fn history_kinds(&self) -> &'static [CanvasHistoryKind] {
        match self {
            Self::Canvas => &[CanvasHistoryKind::Transform, CanvasHistoryKind::EditText],
            Self::Arrange => &[CanvasHistoryKind::Arrange],
            Self::Properties => &[CanvasHistoryKind::EditProperties],
            Self::Adjustments => &[CanvasHistoryKind::AdjustPhoto],
            Self::Layers => &[CanvasHistoryKind::EditLayers],
            Self::QuickLayout => &[CanvasHistoryKind::QuickLayout],
            Self::Gallery | Self::Pages | Self::Templates => &[],
        }
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
        self.finish_inactive_history_scopes();

        self.tree.ui(
            &mut ViewerTreeBehavior {
                scene_state: &mut self.state,
                navigator: &mut navigator,
            },
            ui,
        );
        self.finish_inactive_history_scopes();

        let navigation_request = navigator.process_pending_request();
        if navigation_request.is_some() {
            self.state.finish_selected_history();
        }
        match navigation_request {
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
            self.state
                .apply_page_edit(page_id, CanvasHistoryKind::Crop, |page| {
                    page.apply_crop(layer_id, crop);
                });
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
                        self.scene_state
                            .apply_selected_edit(CanvasHistoryKind::AddPhoto, |page| {
                                page.replace_selected_template_photo(photo.clone())
                            });
                    } else {
                        self.scene_state
                            .apply_selected_edit(CanvasHistoryKind::AddPhoto, |page| {
                                page.add_photo(photo.clone())
                            });
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

                let (page, history) = self.scene_state.selected_page_and_history_mut();
                let response = CanvasArrange { canvas_state: page }.show(ui);
                history.record(CanvasHistoryKind::Arrange, page, response);
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

                let (page, history) = self.scene_state.selected_page_and_history_mut();
                let response = CanvasProperties { canvas_state: page }.show(ui);
                history.record(CanvasHistoryKind::EditProperties, page, response);
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

                let page_id = self.scene_state.pages_state.selected_page;
                let page = self
                    .scene_state
                    .pages_state
                    .pages
                    .get_mut(&page_id)
                    .unwrap();
                let history = self.scene_state.history_managers.get_mut(&page_id).unwrap();
                let adjustments_state = &mut self.scene_state.adjustments_state;
                let response = CanvasAdjustments {
                    canvas_state: page,
                    adjustments_state,
                }
                .show(ui);
                history.record(CanvasHistoryKind::AdjustPhoto, page, response);
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

                let (page, history) = self.scene_state.selected_page_and_history_mut();
                let response = CanvasLayers { canvas_state: page }.show(ui);
                history.record(CanvasHistoryKind::EditLayers, page, response);
            }
            CanvasScenePane::Pages => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, color::SIDE_PANEL_BACKGROUND);

                match Pages::new(&mut self.scene_state.pages_state).show(ui) {
                    PagesResponse::SelectPage(page_id) => {
                        self.scene_state.select_page(page_id);
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

                        self.scene_state.sync_history_managers();
                        self.scene_state.select_page(new_page_id);
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
                let response = QuickLayout::new(&mut quick_layout_state, page).show(ui);
                history.record(CanvasHistoryKind::QuickLayout, page, response);
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
        ui.input(|input| {
            if input.key_pressed(Key::C) && input.modifiers.shift {
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

            if input.key_pressed(Key::V) && input.modifiers.shift {
                let clipboard_content = self.scene_state.clipboard.clone();

                if let Some(clipboard_layers) = clipboard_content {
                    let page_id = self.scene_state.pages_state.selected_page;
                    self.scene_state
                        .apply_page_edit(page_id, CanvasHistoryKind::Paste, |page| {
                            page.paste_layers(clipboard_layers, Vec2::new(20.0, 20.0))
                        });
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        state.apply_page_edit(first_page_id, CanvasHistoryKind::AddShape, |page| {
            page.add_layer(Layer::new_rectangle_shape_layer(), false);
        });
        state.apply_page_edit(first_page_id, CanvasHistoryKind::AddShape, |page| {
            page.add_layer(Layer::new_rectangle_shape_layer(), false);
        });
        state.apply_page_edit(second_page_id, CanvasHistoryKind::AddShape, |page| {
            page.add_layer(Layer::new_rectangle_shape_layer(), false);
        });

        assert_eq!(state.history_managers[&first_page_id].history_len(), 2);
        assert_eq!(state.history_managers[&second_page_id].history_len(), 1);

        let first_page = state.pages_state.pages.get_mut(&first_page_id).unwrap();
        state
            .history_managers
            .get_mut(&first_page_id)
            .unwrap()
            .undo(first_page);

        assert_eq!(state.pages_state.pages[&first_page_id].layers.len(), 1);
        assert_eq!(state.pages_state.pages[&second_page_id].layers.len(), 1);
        assert_eq!(state.history_managers[&second_page_id].index(), 1);
    }

    #[test]
    fn selecting_another_page_finishes_the_pending_edit() {
        let first_page_id = next_page_id();
        let second_page_id = next_page_id();
        let mut state = CanvasSceneState::with_pages(
            indexmap! {
                first_page_id => CanvasState::new(),
                second_page_id => CanvasState::new(),
            },
            first_page_id,
        );

        let (page, history) = state.selected_page_and_history_mut();
        page.add_layer(Layer::new_rectangle_shape_layer(), false);
        history.update(CanvasHistoryKind::EditProperties, page);
        assert_eq!(history.history_len(), 0);

        state.select_page(second_page_id);

        assert_eq!(state.pages_state.selected_page, second_page_id);
        assert_eq!(state.history_managers[&first_page_id].history_len(), 1);
    }

    #[test]
    fn hiding_an_editor_pane_finishes_its_pending_edit() {
        let mut scene = CanvasScene::new();
        let (page, history) = scene.state.selected_page_and_history_mut();
        page.add_layer(Layer::new_rectangle_shape_layer(), false);
        history.update(CanvasHistoryKind::EditProperties, page);
        assert_eq!(history.history_len(), 0);

        scene.set_right_sidebar_open(false);

        let selected_page = scene.state.pages_state.selected_page;
        assert_eq!(
            scene.state.history_managers[&selected_page].history_len(),
            1
        );
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
