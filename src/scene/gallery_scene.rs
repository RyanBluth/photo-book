use egui::{Ui, widget_text::WidgetText};
use egui_tiles::{Behavior, Linear, LinearDir, TileId, Tiles, Tree, UiResponse};

use crate::{
    dep,
    photo::SaveOnDropPhoto,
    photo_manager::PhotoManager,
    selection_manager::SelectionManager,
    utils::EguiUiExt,
    widget::{
        image_gallery::{ImageGallery, ImageGalleryState},
        photo_info::{PhotoInfo, PhotoInfoState},
    },
};

use super::{
    NavigationRequest, Navigator, Scene, SceneResponse, SceneTransition, viewer_scene::ViewerScene,
};

#[derive(Debug)]
pub struct GallerySceneState {
    pub image_gallery_state: ImageGalleryState,
    pub photo_info_state: PhotoInfoState,
}

impl Default for GallerySceneState {
    fn default() -> Self {
        Self {
            image_gallery_state: ImageGalleryState::default(),
            photo_info_state: PhotoInfoState::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GalleryScenePane {
    Gallery,
    PhotoInfo,
}

#[derive(Debug)]
pub struct GalleryScene {
    pub state: GallerySceneState,
    tree: Tree<GalleryScenePane>,
}

impl GalleryScene {
    pub fn new() -> Self {
        let mut tiles = Tiles::default();

        let gallery_pane_id = tiles.insert_pane(GalleryScenePane::Gallery);
        let photo_info_id = tiles.insert_pane(GalleryScenePane::PhotoInfo);

        let mut linear_layout =
            Linear::new(LinearDir::Horizontal, vec![gallery_pane_id, photo_info_id]);
        linear_layout.shares.set_share(photo_info_id, 0.2);

        Self {
            state: GallerySceneState::default(),
            tree: Tree::new(
                "organize_scene_tree",
                tiles.insert_container(linear_layout),
                tiles,
            ),
        }
    }

    pub fn set_right_sidebar_open(&mut self, open: bool) {
        if let Some(tile_id) = self.tree.tiles.find_pane(&GalleryScenePane::PhotoInfo) {
            self.tree.tiles.set_visible(tile_id, open);
        }
    }
}

impl Scene for GalleryScene {
    fn ui(&mut self, ui: &mut Ui) -> SceneResponse {
        let mut navigator = Navigator::new();

        self.tree.ui(
            &mut GalleryTreeBehavior {
                scene_state: &mut self.state,
                navigator: &mut navigator,
            },
            ui,
        );

        match navigator.process_pending_request() {
            Some(NavigationRequest::Push(scene)) => SceneResponse::Push(scene),
            Some(NavigationRequest::Pop(response)) => SceneResponse::Pop(response),
            None => SceneResponse::None,
        }
    }
}

struct GalleryTreeBehavior<'a> {
    scene_state: &'a mut GallerySceneState,
    navigator: &'a mut Navigator,
}

impl GalleryTreeBehavior<'_> {
    fn scroll_to_photo_path(&mut self) -> Option<std::path::PathBuf> {
        let selection_change = dep!(SelectionManager, |selection_manager| {
            selection_manager.last_frame_selection()
        })?;

        selection_change.added_paths.first().cloned()
    }

    fn ui(&mut self, pane: &GalleryScenePane, ui: &mut Ui) -> UiResponse {
        match pane {
            GalleryScenePane::Gallery => {
                let scroll_to_photo_path = self.scroll_to_photo_path();

                // Show the gallery with any scroll-to info from the state
                let gallery_response = ImageGallery::show(
                    ui,
                    &mut self.scene_state.image_gallery_state,
                    scroll_to_photo_path.as_ref(),
                );

                // Handle gallery responses
                if let Some(photo) = gallery_response.primary_action_photo {
                    // Handle primary action (double-click) - open viewer
                    self.navigator
                        .push(SceneTransition::Viewer(ViewerScene::new(photo)));
                }

                UiResponse::None
            }
            GalleryScenePane::PhotoInfo => {
                let selected_images = dep!(SelectionManager, |selection_manager| {
                    selection_manager.selected_paths().clone()
                });

                match selected_images.len() {
                    1 => {
                        let Some(selected_image) = selected_images.iter().next() else {
                            return UiResponse::None;
                        };
                        let mut photo = dep!(PhotoManager, |photo_manager| {
                            photo_manager
                                .photo_database
                                .get_photo(selected_image)
                                .cloned()
                        });

                        if let Some(photo) = &mut photo {
                            PhotoInfo::new(
                                SaveOnDropPhoto::new(photo),
                                &mut self.scene_state.photo_info_state,
                            )
                            .show(ui);
                        } else {
                            ui.both_centered(|ui| {
                                ui.heading("Nothing selected");
                            });
                        }
                    }
                    0 => {
                        ui.both_centered(|ui| {
                            ui.heading("Nothing selected");
                        });
                    }
                    _ => {
                        ui.both_centered(|ui| {
                            ui.heading("Multiple photos selected");
                        });
                    }
                }

                UiResponse::None
            }
        }
    }
}

impl Behavior<GalleryScenePane> for GalleryTreeBehavior<'_> {
    fn pane_ui(
        &mut self,
        ui: &mut Ui,
        _tile_id: TileId,
        pane: &mut GalleryScenePane,
    ) -> UiResponse {
        self.ui(pane, ui)
    }

    fn tab_title_for_pane(&mut self, pane: &GalleryScenePane) -> WidgetText {
        match pane {
            GalleryScenePane::Gallery => "Gallery".into(),
            GalleryScenePane::PhotoInfo => "Photo Info".into(),
        }
    }
}
