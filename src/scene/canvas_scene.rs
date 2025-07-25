use std::fmt::Display;

use egui::{Id, Key, Rect, Ui, Vec2};
use egui_tiles::UiResponse;
use indexmap::{indexmap, IndexMap};

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    export::{ExportTaskId, ExportTaskStatus, Exporter},
    history::{HistoricallyEqual, UndoRedoStack},
    id::{next_layer_id, next_page_id, LayerId, PageId},

    model::edit_state::EditablePage,
    photo_manager::PhotoManager,
    scene::crop_scene::CropScene,
    utils::{IdExt, RectExt},
    widget::{
        canvas::{Canvas, CanvasPhoto, CanvasState, MultiSelect},
        canvas_info::{
            layers::{Layer, LayerContent},
            panel::CanvasInfo,
            quick_layout::{QuickLayout, QuickLayoutState},
        },
        image_gallery::{ImageGallery, ImageGalleryState},
        pages::{Pages, PagesResponse, PagesState},
        templates::{Templates, TemplatesResponse, TemplatesState},
    },
};

use super::{
    crop_scene::CropSceneResponse, viewer_scene::ViewerScene, NavigationRequest, Navigator, Scene,
    ScenePopResponse, SceneResponse, SceneTransition::Viewer,
};

use crate::widget::canvas::CanvasResponse;

#[derive(Debug, Clone)]
pub struct CanvasSceneState {
    pub gallery_state: ImageGalleryState,
    pub pages_state: PagesState,
    history_manager: CanvasHistoryManager,
    templates_state: TemplatesState,
    export_task_id: Option<ExportTaskId>,
    quick_layout_state: QuickLayoutState,
    pub clipboard: Option<Vec<Layer>>,
}

impl CanvasSceneState {
    pub fn new() -> Self {
        let page_id = next_page_id();
        let initial_state = CanvasState::new();

        Self {
            gallery_state: ImageGalleryState::default(),
            history_manager: CanvasHistoryManager::with_initial_state(initial_state.clone()),
            pages_state: PagesState::new(indexmap! { page_id => initial_state }, page_id),
            templates_state: TemplatesState::new(),
            export_task_id: None,
            quick_layout_state: QuickLayoutState::new(),
            clipboard: None,
        }
    }

    pub fn with_pages(pages: IndexMap<PageId, CanvasState>, selected_page: PageId) -> Self {
        Self {
            gallery_state: ImageGalleryState::default(),
            history_manager: CanvasHistoryManager::with_initial_state(
                pages[&selected_page].clone(),
            ),
            pages_state: PagesState::new(pages, selected_page),
            templates_state: TemplatesState::new(),
            export_task_id: None,
            quick_layout_state: QuickLayoutState::new(),
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
        let page = self
            .pages_state
            .pages
            .get_mut(&self.pages_state.selected_page)
            .unwrap();
        (&mut *page, &mut self.history_manager)
    }

    pub fn has_pages(&self) -> bool {
        !self.pages_state.pages.is_empty()
    }
}

#[derive(Debug, Clone)]
pub enum CanvasScenePane {
    Gallery,
    Canvas,
    Info,
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
    pub fn new() -> Self {
        let mut tiles = egui_tiles::Tiles::default();

        let left_tabs = vec![
            tiles.insert_pane(CanvasScenePane::Gallery),
            tiles.insert_pane(CanvasScenePane::Pages),
            tiles.insert_pane(CanvasScenePane::Templates),
        ];

        let left_tabs_ids = tiles.insert_tab_tile(left_tabs);
        let canvas_id = tiles.insert_pane(CanvasScenePane::Canvas);

        let right_tabs = vec![
            tiles.insert_pane(CanvasScenePane::Info),
            tiles.insert_pane(CanvasScenePane::QuickLayout),
        ];
        let right_tabs_id = tiles.insert_tab_tile(right_tabs);

        let children = vec![left_tabs_ids, canvas_id, right_tabs_id];

        let mut linear_layout =
            egui_tiles::Linear::new(egui_tiles::LinearDir::Horizontal, children);
        linear_layout.shares.set_share(left_tabs_ids, 0.2);
        linear_layout.shares.set_share(right_tabs_id, 0.2);

        Self {
            state: CanvasSceneState::new(),
            tree: egui_tiles::Tree::new(
                "canvas_scene_tree",
                tiles.insert_container(linear_layout),
                tiles,
            ),
        }
    }

    pub fn with_state(state: CanvasSceneState) -> Self {
        let mut res = Self::new();
        res.state = state;
        res
    }
}

impl Scene for CanvasScene {
    fn ui(&mut self, ui: &mut egui::Ui) -> SceneResponse {
        // Remove the sync code since we're working directly with the selected page

        match self.state.export_task_id {
            Some(task_id) => {
                let exporter: Singleton<Exporter> = Dependency::get();
                let status = exporter.with_lock(|exporter| exporter.get_task_status(task_id));

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
                    let exporter: Singleton<Exporter> = Dependency::get();
                    self.state.export_task_id = Some(exporter.with_lock_mut(|exporter| {
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

        self.tree.ui(
            &mut ViewerTreeBehavior {
                scene_state: &mut self.state,
                navigator: &mut navigator,
            },
            ui,
        );

        match navigator.process_pending_request() {
            Some(NavigationRequest::Push(scene_state)) => SceneResponse::Push(scene_state),
            Some(NavigationRequest::Pop(response)) => SceneResponse::Pop(response),
            None => SceneResponse::None,
        }
    }

    fn popped(&mut self, popped_scene_response: ScenePopResponse) {
        match popped_scene_response {
            ScenePopResponse::Crop(CropSceneResponse::Apply {
                layer_id,
                page_id,
                crop,
            }) => {
                let page = self.state.pages_state.pages.get_mut(&page_id).unwrap();
                let layer = page.layers.get_mut(&layer_id).unwrap();
                if let LayerContent::Photo(photo) = &mut layer.content {
                    photo.crop = crop;

                    let rotated_crop =
                        crop.rotate_bb_around_center(photo.photo.metadata.rotation().radians());

                    let photo_rect: Rect = Rect::from_center_size(
                        layer.transform_state.rect.center(),
                        Vec2::new(
                            photo.photo.metadata.rotated_width() as f32 * rotated_crop.size().x,
                            photo.photo.metadata.rotated_height() as f32 * rotated_crop.size().y,
                        ),
                    );

                    layer.transform_state.rect =
                        photo_rect.fit_and_center_within(layer.transform_state.rect);
                }
            }
            _ => {}
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
                    .rect_filled(ui.max_rect(), 0.0, ui.style().visuals.panel_fill);
                let response = ImageGallery::show(
                    ui,
                    &mut self.scene_state.gallery_state,
                    None
                );

                // Handle primary action (double-click)
                if let Some(photo) = response.primary_action_photo {
                    let is_template = self.scene_state.selected_page().template.is_some();

                    if is_template {
                        let page = self.scene_state.selected_page_mut();
                        let mut selected_template_photos: Vec<_> = page
                            .layers
                            .iter_mut()
                            .filter(|(_, layer)| {
                                matches!(layer.content, LayerContent::TemplatePhoto { .. })
                                    && layer.selected
                            })
                            .collect();

                        if selected_template_photos.len() == 1 {
                            if let LayerContent::TemplatePhoto {
                                region: _,
                                photo: canvas_photo,
                                scale_mode: _,
                            } = &mut selected_template_photos[0].1.content
                            {
                                *canvas_photo = Some(CanvasPhoto::new(photo.clone()));
                            }
                            // Create a snapshot of the state after modification
                            let page_snapshot = self.scene_state.selected_page().clone();
                            self.scene_state
                                .history_manager
                                .save_history(CanvasHistoryKind::AddPhoto, &page_snapshot);
                        }
                    } else {
                        self.scene_state
                            .selected_page_mut()
                            .add_photo(photo.clone());
                        // Create a snapshot of the state after modification
                        let page_snapshot = self.scene_state.selected_page().clone();
                        self.scene_state
                            .history_manager
                            .save_history(CanvasHistoryKind::AddPhoto, &page_snapshot);
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
                let (page, history) = self.scene_state.selected_page_and_history_mut();

                match Canvas::new(page, rect, history).show(ui) {
                    Some(CanvasResponse::EnterCropMode {
                        target_layer,
                        photo,
                    }) => {
                        let crop_scene = CropScene::new(
                            target_layer,
                            self.scene_state.pages_state.selected_page,
                            rect,
                            photo.photo,
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
            CanvasScenePane::Info => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, ui.style().visuals.panel_fill);

                if !self.scene_state.has_pages() {
                    ui.centered_and_justified(|ui| {
                        ui.heading("No page selected");
                    });
                    return UiResponse::None;
                }

                let (page, history) = self.scene_state.selected_page_and_history_mut();
                let response: egui::InnerResponse<
                    crate::widget::canvas_info::panel::CanvasInfoResponse,
                > = CanvasInfo {
                    canvas_state: page,
                    history_manager: history,
                }
                .show(ui);

                if let Some(history_kind) = response.inner.history {
                    let page_snapshot = self.scene_state.selected_page().clone();
                    self.scene_state
                        .history_manager
                        .save_history(history_kind, &page_snapshot);
                }
            }
            CanvasScenePane::Pages => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, ui.style().visuals.panel_fill);

                match Pages::new(&mut self.scene_state.pages_state).show(ui) {
                    PagesResponse::SelectPage => {
                        // No need to sync canvas_state anymore
                    }
                    PagesResponse::None => {}
                }
            }
            CanvasScenePane::Templates => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, ui.style().visuals.panel_fill);

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
            CanvasScenePane::Info => "Info".into(),
            CanvasScenePane::Pages => "Pages".into(),
            CanvasScenePane::Templates => "Templates".into(),
            CanvasScenePane::QuickLayout => "Quick Layout".into(),
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

                    let page_snapshot = self
                        .scene_state
                        .pages_state
                        .pages
                        .get(&page_id)
                        .unwrap()
                        .clone();
                    self.scene_state
                        .history_manager
                        .save_history(CanvasHistoryKind::AddPhoto, &page_snapshot);
                }
            }
        });
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanvasHistoryKind {
    Transform,
    AddPhoto,
    DeletePhoto,
    Select,
    Page, // TODO Add specific cases for things within the page settings
    AddText,
    EditText,
    SelectLayer,
    DeselectLayer,
    QuickLayout,
}

impl Display for CanvasHistoryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CanvasHistoryKind::Transform => write!(f, "Move"),
            CanvasHistoryKind::AddPhoto => write!(f, "Add Photo"),
            CanvasHistoryKind::DeletePhoto => write!(f, "Delete Photo"),
            CanvasHistoryKind::Select => write!(f, "Select"),
            CanvasHistoryKind::Page => write!(f, "Page"),
            CanvasHistoryKind::AddText => write!(f, "Add Text"),
            CanvasHistoryKind::EditText => write!(f, "Edit Text"),
            CanvasHistoryKind::SelectLayer => write!(f, "Select Layer"),
            CanvasHistoryKind::DeselectLayer => write!(f, "Deselect Layer"),
            CanvasHistoryKind::QuickLayout => write!(f, "Quick Layout"),
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

    pub fn is_at_end(&self) -> bool {
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

    pub fn apply_index(&mut self, index: usize, canvas_state: &mut CanvasState) {
        let history = &self.stack.history[index];
        self.apply_history(history.1.clone(), canvas_state);
    }

    pub fn capturing_history<T>(
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
