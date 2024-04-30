use std::fmt::Display;

use egui::Ui;
use egui_tiles::UiResponse;
use indexmap::{indexmap, IndexMap};

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    history::{HistoricallyEqual, UndoRedoStack},
    id::{next_page_id, LayerId},
    model::{edit_state::EditablePage, page::Page},
    photo::Photo,
    photo_manager::{self, PhotoManager},
    widget::{
        canvas_info::{layers::Layer, panel::CanvasInfo},
        image_gallery::{ImageGallery, ImageGalleryResponse, ImageGalleryState},
        page_canvas::{Canvas, CanvasState, MultiSelect},
        pages::{Pages, PagesResponse, PagesState},
        templates::{Templates, TemplatesResponse, TemplatesState},
    },
};

use super::{viewer_scene::ViewerScene, NavigationRequest, Navigator, Scene, SceneResponse};

pub struct CanvasSceneState {
    canvas_state: CanvasState,
    gallery_state: ImageGalleryState,
    pages_state: PagesState,
    history_manager: CanvasHistoryManager,
    templates_state: TemplatesState,
}

impl CanvasSceneState {
    fn new() -> Self {
        let page_id = next_page_id();

        Self {
            canvas_state: CanvasState::new(),
            gallery_state: ImageGalleryState::default(),
            history_manager: CanvasHistoryManager::new(),
            pages_state: PagesState::new(indexmap! { page_id => CanvasState::new() }, page_id),
            templates_state: TemplatesState::new(),
        }
    }

    fn with_photo(photo: Photo, gallery_state: Option<ImageGalleryState>) -> Self {
        let canvas_state = CanvasState::with_photo(photo.clone(), ImageGalleryState::default());
        let page_id = next_page_id();

        Self {
            canvas_state: canvas_state.clone(),
            gallery_state: gallery_state.unwrap_or_default(),
            history_manager: CanvasHistoryManager::with_initial_state(canvas_state.clone()),
            pages_state: PagesState::new(indexmap! { page_id => canvas_state }, page_id),
            templates_state: TemplatesState::new(),
        }
    }
}

pub enum CanvasScenePane {
    Gallery,
    Canvas,
    Info,
    Pages,
    Templates,
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
            tiles.insert_pane(CanvasScenePane::Templates),
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
        ui: &mut Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut CanvasScenePane,
    ) -> UiResponse {
        match pane {
            CanvasScenePane::Gallery => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, ui.style().visuals.panel_fill);
                match ImageGallery::show(ui, &mut self.scene_state.gallery_state) {
                    Some(response) => match response {
                        ImageGalleryResponse::ViewPhotoAt(index) => {
                            let photo_manager: Singleton<PhotoManager> = Dependency::get();
                            if let (index, Some(photo_result)) =
                                photo_manager.with_lock(|photo_manager| {
                                    (
                                        index,
                                        photo_manager.photos.get_index(index).map(|x| x.1.clone()),
                                    )
                                })
                            {
                                match photo_result {
                                    photo_manager::PhotoLoadResult::Pending(path) => todo!(),
                                    photo_manager::PhotoLoadResult::Ready(photo) => self
                                        .navigator
                                        .push(Box::new(ViewerScene::new(photo, index))),
                                }
                            }
                        }
                        ImageGalleryResponse::EditPhotoAt(index) => {
                            let photo_manager: Singleton<PhotoManager> = Dependency::get();
                            if let Some(photo_result) = photo_manager.with_lock(|photo_manager| {
                                photo_manager.photos.get_index(index).map(|x| x.1.clone())
                            }) {
                                match photo_result {
                                    photo_manager::PhotoLoadResult::Pending(path) => todo!(),
                                    photo_manager::PhotoLoadResult::Ready(photo) => {
                                        let layer = Layer::with_photo(photo.clone());
                                        self.scene_state
                                            .canvas_state
                                            .layers
                                            .insert(layer.id, layer);
                                        self.scene_state.history_manager.save_history(
                                            CanvasHistoryKind::AddPhoto,
                                            &mut self.scene_state.canvas_state,
                                        );
                                    }
                                }
                            }
                        }
                    },
                    None => {}
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
            }
            CanvasScenePane::Pages => {
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, ui.style().visuals.panel_fill);

                self.scene_state.pages_state.pages.insert(
                    self.scene_state.pages_state.selected_page,
                    self.scene_state.canvas_state.clone_with_new_widget_ids(),
                );

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
                    TemplatesResponse::SelectTemplate => {}
                    TemplatesResponse::None => {}
                }
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
