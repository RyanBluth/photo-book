use std::fmt::Display;

use egui::{Color32, Widget};
use egui_tiles::{Tabs, UiResponse};
use indexmap::{indexmap, IndexMap};
use rayon::vec;

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    history::{HistoricallyEqual, UndoRedoStack},
    photo::Photo,
    photo_manager::PhotoManager,
    widget::{
        canvas_info::{
            layers::{next_layer_id, Layer, LayerId},
            panel::CanvasInfo,
        },
        image_gallery::{ImageGallery, ImageGalleryState},
        image_viewer::{self, ImageViewer, ImageViewerState},
        page_canvas::{Canvas, CanvasState, MultiSelect, Page},
        pages::{Pages, PagesState},
        photo_info::PhotoInfo,
    },
};

use super::{NavigationRequest, Navigator, Scene, SceneResponse};

pub struct CanvasSceneState {
    canvas_state: CanvasState,
    gallery_state: ImageGalleryState,
    pages_state: PagesState,
    history_manager: CanvasHistoryManager,
}

impl CanvasSceneState {
    fn new() -> Self {
        Self {
            canvas_state: CanvasState::new(),
            gallery_state: ImageGalleryState::default(),
            history_manager: CanvasHistoryManager::new(),
            pages_state: PagesState::new(vec![CanvasState::new()]),
        }
    }

    fn with_photo(photo: Photo, gallery_state: Option<ImageGalleryState>) -> Self {
        let canvas_state = CanvasState::with_photo(photo.clone(), ImageGalleryState::default());

        Self {
            canvas_state: canvas_state.clone(),
            gallery_state: gallery_state.unwrap_or_default(),
            history_manager: CanvasHistoryManager::with_initial_state(canvas_state.clone()),
            pages_state: PagesState::new(vec![canvas_state]),
        }
    }
}

pub enum CanvasScenePane {
    Gallery,
    Canvas,
    Info,
    Pages,
}

pub struct CanvasScene {
    state: CanvasSceneState,
    tree: egui_tiles::Tree<CanvasScenePane>,
}

impl CanvasScene {
    pub fn with_photo(photo: Photo, gallery_state: Option<ImageGalleryState>) -> Self {
        let mut tiles = egui_tiles::Tiles::default();

        let tabs = vec![
            tiles.insert_pane(CanvasScenePane::Gallery),
            tiles.insert_pane(CanvasScenePane::Pages),
        ];

        let tabs_id = tiles.insert_tab_tile(tabs);
        let canvas_id = tiles.insert_pane(CanvasScenePane::Canvas);
        let info_id = tiles.insert_pane(CanvasScenePane::Info);

        let children = vec![tabs_id, canvas_id, info_id];

        let mut linear_layout =
            egui_tiles::Linear::new(egui_tiles::LinearDir::Horizontal, children);
        linear_layout.shares.set_share(tabs_id, 0.2);
        linear_layout.shares.set_share(info_id, 0.2);

        Self {
            state: CanvasSceneState::with_photo(photo, gallery_state),
            tree: egui_tiles::Tree::new(
                "canvas_scene_tree",
                tiles.insert_container(linear_layout),
                tiles,
            ),
        }
    }
}

impl Scene for CanvasScene {
    fn ui(&mut self, ui: &mut egui::Ui) -> SceneResponse {
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
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut CanvasScenePane,
    ) -> UiResponse {
        match pane {
            CanvasScenePane::Gallery => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, ui.style().visuals.panel_fill);
                ImageGallery::show(ui, &mut self.scene_state.gallery_state);
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
                let pre_info_canvas_state = self.scene_state.canvas_state.clone();

                let response = CanvasInfo {
                    layers: &mut self.scene_state.canvas_state.layers,
                    page: &mut self.scene_state.canvas_state.page,
                    history_manager: &mut self.scene_state.history_manager,
                }
                .show(ui);

                if let Some(history) = response.inner.history {
                    self.scene_state
                        .history_manager
                        .save_history(history, &self.scene_state.canvas_state);
                }
                // },
                // );
            }
            CanvasScenePane::Pages => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, ui.style().visuals.panel_fill);

                Pages::new(&mut self.scene_state.pages_state).show(ui);
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
    page: Page,
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
                        page: Page::default(),
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
