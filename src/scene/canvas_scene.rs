use std::fmt::Display;

use egui::{Key, Ui};
use egui_tiles::UiResponse;
use indexmap::{indexmap, IndexMap};

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    export::{ExportTaskId, ExportTaskStatus, Exporter},
    history::{HistoricallyEqual, UndoRedoStack},
    id::{next_page_id, LayerId, PageId},
    model::{edit_state::EditablePage, page::Page},
    widget::{
        canvas_info::{
            layers::{Layer, LayerContent},
            panel::CanvasInfo,
            quick_layout::{QuickLayout, QuickLayoutState},
        },
        image_gallery::{ImageGallery, ImageGalleryResponse, ImageGalleryState},
        page_canvas::{Canvas, CanvasPhoto, CanvasState, MultiSelect},
        pages::{Pages, PagesResponse, PagesState},
        templates::{Templates, TemplatesResponse, TemplatesState},
    },
};

use super::{
    viewer_scene::ViewerScene, NavigationRequest, Navigator, Scene, SceneResponse,
    SceneTransition::Viewer,
};

#[derive(Debug, Clone)]
pub struct CanvasSceneState {
    pub canvas_state: CanvasState,
    pub gallery_state: ImageGalleryState,
    pub pages_state: PagesState,
    history_manager: CanvasHistoryManager,
    templates_state: TemplatesState,
    export_task_id: Option<ExportTaskId>,
}

impl CanvasSceneState {
    pub fn new() -> Self {
        let page_id = next_page_id();

        Self {
            canvas_state: CanvasState::new(),
            gallery_state: ImageGalleryState::default(),
            history_manager: CanvasHistoryManager::new(),
            pages_state: PagesState::new(indexmap! { page_id => CanvasState::new() }, page_id),
            templates_state: TemplatesState::new(),
            export_task_id: None,
        }
    }

    pub fn with_pages(pages: IndexMap<PageId, CanvasState>, selected_page: PageId) -> Self {
        Self {
            canvas_state: pages[&selected_page].clone(),
            gallery_state: ImageGalleryState::default(),
            history_manager: CanvasHistoryManager::new(),
            pages_state: PagesState::new(pages, selected_page),
            templates_state: TemplatesState::new(),
            export_task_id: None,
        }
    }
}

pub enum CanvasScenePane {
    Gallery,
    Canvas,
    Info,
    Pages,
    Templates,
    QuickLayout,
}

pub struct CanvasScene {
    pub state: CanvasSceneState,
    tree: egui_tiles::Tree<CanvasScenePane>,
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
        // Apply the current canvas state to the pages state so they are in sync
        self.state.pages_state.pages.insert(
            self.state.pages_state.selected_page,
            self.state.canvas_state.clone_with_new_widget_ids(),
        );

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
            Some(NavigationRequest::Pop) => SceneResponse::Pop,
            None => SceneResponse::None,
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
        ui: &mut Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut CanvasScenePane,
    ) -> UiResponse {
        match pane {
            CanvasScenePane::Gallery => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, ui.style().visuals.panel_fill);
                if let Some(response) = ImageGallery::show(ui, &mut self.scene_state.gallery_state)
                {
                    match response {
                        ImageGalleryResponse::SelectPhotoSecondaryAction(photo) => {
                            self.navigator.push(Viewer(ViewerScene::new(photo.clone())));
                        }
                        ImageGalleryResponse::SelectPhotoPrimaryAction(photo) => {
                            let is_template = self.scene_state.canvas_state.template.is_some();

                            if is_template {
                                let mut selected_template_photos: Vec<_> = self
                                    .scene_state
                                    .canvas_state
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

                                    self.scene_state.history_manager.save_history(
                                        CanvasHistoryKind::AddPhoto,
                                        &mut self.scene_state.canvas_state,
                                    );
                                } else if selected_template_photos.len() > 1
                                    || selected_template_photos.is_empty()
                                {
                                    // TODO: Show error message saying that only one template photo can be selected
                                }
                            } else {
                                let layer = Layer::with_photo(photo.clone());
                                self.scene_state.canvas_state.layers.insert(layer.id, layer);
                                self.scene_state.history_manager.save_history(
                                    CanvasHistoryKind::AddPhoto,
                                    &mut self.scene_state.canvas_state,
                                );
                            }
                        }
                    }
                }
            }
            CanvasScenePane::Canvas => {
                Canvas::new(
                    &mut self.scene_state.canvas_state,
                    ui.available_rect_before_wrap(),
                    &mut self.scene_state.history_manager,
                )
                .show(ui);
            }
            CanvasScenePane::Info => {
                // self.scene_state.history_manager.capturing_history(
                //     CanvasHistoryKind::AddPhoto,
                //     &mut self.scene_state.canvas_state,
                //     |canvas_state| {

                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, ui.style().visuals.panel_fill);

                // Clone the state so that we can use it to save history if needed.
                // History needs to be saved before the state is modified
                let _pre_info_canvas_state = self.scene_state.canvas_state.clone();

                let response = CanvasInfo {
                    canvas_state: &mut self.scene_state.canvas_state,
                    history_manager: &mut self.scene_state.history_manager,
                }
                .show(ui);

                if let Some(history) = response.inner.history {
                    self.scene_state
                        .history_manager
                        .save_history(history, &self.scene_state.canvas_state);
                }
            }
            CanvasScenePane::Pages => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, ui.style().visuals.panel_fill);

                match Pages::new(&mut self.scene_state.pages_state).show(ui) {
                    PagesResponse::SelectPage => {
                        self.scene_state.canvas_state = self
                            .scene_state
                            .pages_state
                            .pages
                            .get_key_value(&self.scene_state.pages_state.selected_page)
                            .unwrap()
                            .1
                            .clone_with_new_widget_ids();
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
                            .insert(new_page_id, new_canvas_state.clone());

                        self.scene_state.pages_state.selected_page = new_page_id;

                        self.scene_state.canvas_state = self
                            .scene_state
                            .pages_state
                            .pages
                            .get_key_value(&self.scene_state.pages_state.selected_page)
                            .unwrap()
                            .1
                            .clone_with_new_widget_ids();
                    }
                    TemplatesResponse::None => {}
                }
            }
            CanvasScenePane::QuickLayout => {
                QuickLayout::new(&mut QuickLayoutState::new(
                    &mut self.scene_state.canvas_state,
                    &mut self.scene_state.history_manager,
                ))
                .show(ui);
            }
        }

        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &CanvasScenePane) -> egui::WidgetText {
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

#[derive(Debug, Clone, PartialEq)]
pub enum CanvasHistoryKind {
    Initial,
    Transform,
    AddPhoto,
    DeletePhoto,
    Select,
    Page, // TODO Add specific cases for things within the page settings
    AddText,
    SelectLayer,
    DeselectLayer,
    QuickLayout,
}

impl Display for CanvasHistoryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CanvasHistoryKind::Initial => write!(f, "Initial"),
            CanvasHistoryKind::Transform => write!(f, "Move"),
            CanvasHistoryKind::AddPhoto => write!(f, "Add Photo"),
            CanvasHistoryKind::DeletePhoto => write!(f, "Delete Photo"),
            CanvasHistoryKind::Select => write!(f, "Select"),
            CanvasHistoryKind::Page => write!(f, "Page"),
            CanvasHistoryKind::AddText => write!(f, "Add Text"),
            CanvasHistoryKind::SelectLayer => write!(f, "Select Layer"),
            CanvasHistoryKind::DeselectLayer => write!(f, "Deselect Layer"),
            CanvasHistoryKind::QuickLayout => write!(f, "Quick Layout"),
        }
    }
}

impl HistoricallyEqual for CanvasHistory {
    fn historically_eqaul_to(&self, other: &Self) -> bool {
        self.layers.len() == other.layers.len()
            && self
                .layers
                .values()
                .zip(other.layers.values())
                .all(|(a, b)| a.historically_eqaul_to(b))
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
    pub fn new() -> Self {
        Self {
            stack: UndoRedoStack {
                history: vec![(
                    CanvasHistoryKind::Initial,
                    CanvasHistory {
                        layers: IndexMap::new(),
                        multi_select: None,
                        page: EditablePage::new(Page::default()),
                    },
                )],
                index: 0,
            },
        }
    }

    pub fn with_initial_state(state: CanvasState) -> Self {
        Self {
            stack: UndoRedoStack {
                history: vec![(
                    CanvasHistoryKind::Initial,
                    CanvasHistory {
                        layers: state.layers.clone(),
                        multi_select: state.multi_select.clone(),
                        page: state.page.clone(),
                    },
                )],
                index: 0,
            },
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
