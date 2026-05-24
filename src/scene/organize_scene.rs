use std::path::PathBuf;

use egui::{Ui, widget_text::WidgetText};
use egui_tiles::{Behavior, Linear, LinearDir, TileId, Tiles, Tree, UiResponse};

use crate::{
    dep, dep_mut,
    photo::SaveOnDropPhoto,
    photo_manager::PhotoManager,
    selection_manager::SelectionManager,
    utils::EguiUiExt,
    widget::{
        image_gallery::{ImageGallery, ImageGalleryState},
        left_sidebar::{LeftSidebar, LeftSidebarState},
        photo_info::{PhotoInfo, PhotoInfoState},
    },
};

use super::{
    NavigationRequest, Navigator, Scene, SceneResponse, SceneTransition, viewer_scene::ViewerScene,
};

#[derive(Debug, Clone)]
pub struct GallerySceneState {
    pub image_gallery_state: ImageGalleryState,
    pub photo_info_state: PhotoInfoState,
    pub left_sidebar_state: LeftSidebarState,
}

impl Default for GallerySceneState {
    fn default() -> Self {
        Self {
            image_gallery_state: ImageGalleryState::default(),
            photo_info_state: PhotoInfoState::new(),
            left_sidebar_state: LeftSidebarState::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GalleryScenePane {
    Gallery,
    PhotoInfo,
    Sidebar,
}

#[derive(Debug, Clone)]
pub struct GalleryScene {
    pub state: GallerySceneState,
    tree: Tree<GalleryScenePane>,
}

impl GalleryScene {
    pub fn new() -> Self {
        let mut tiles = Tiles::default();

        let gallery_pane_id = tiles.insert_pane(GalleryScenePane::Gallery);

        let right_tabs = vec![tiles.insert_pane(GalleryScenePane::PhotoInfo)];
        let right_tabs_id = tiles.insert_tab_tile(right_tabs);

        let left_tabs = vec![tiles.insert_pane(GalleryScenePane::Sidebar)];
        let left_tabs_id = tiles.insert_tab_tile(left_tabs);

        let mut linear_layout = Linear::new(
            LinearDir::Horizontal,
            vec![left_tabs_id, gallery_pane_id, right_tabs_id],
        );

        linear_layout.shares.set_share(right_tabs_id, 0.2);
        linear_layout.shares.set_share(left_tabs_id, 0.2);

        Self {
            state: GallerySceneState::default(),
            tree: Tree::new(
                "organize_scene_tree",
                tiles.insert_container(linear_layout),
                tiles,
            ),
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
    fn open_photo(&mut self, path: &PathBuf) {
        dep!(PhotoManager, |photo_manager| {
            if let Some(photo) = photo_manager.photo_database.get_photo(path) {
                self.navigator
                    .push(SceneTransition::Viewer(ViewerScene::new(photo.clone())));
            }
        });
    }

    fn scroll_to_photo_path(&mut self) -> Option<PathBuf> {
        let selection_change = dep!(SelectionManager, |selection_manager| {
            selection_manager.last_frame_selection()
        })?;

        for path in &selection_change.added_paths {
            self.expand_file_tree_parent_directories(path);
        }

        selection_change.added_paths.first().cloned()
    }

    fn clear_photo_selection(&mut self) {
        dep_mut!(SelectionManager, |selection_manager| selection_manager
            .clear());
    }

    fn expand_file_tree_parent_directories(&mut self, path: &PathBuf) {
        self.scene_state
            .left_sidebar_state
            .file_tree_state
            .expand_parent_directories(path);
    }

    fn apply_album_filter(&mut self, album_id: String) {
        dep_mut!(PhotoManager, |photo_manager| {
            let mut filter = photo_manager.get_current_filter().clone();
            filter.album = Some(album_id);
            photo_manager.set_current_filter(filter);
        });

        self.clear_photo_selection();
    }

    fn remove_photo(&mut self, path: &PathBuf) {
        dep_mut!(PhotoManager, |photo_manager| {
            photo_manager.photo_database.remove_photo(path);
        });
        dep_mut!(SelectionManager, |selection_manager| {
            selection_manager.remove_path(path)
        });
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
            GalleryScenePane::Sidebar => {
                let scroll_to_photo_path = self.scroll_to_photo_path();

                let sidebar_response = LeftSidebar::new(&mut self.scene_state.left_sidebar_state)
                    .show(ui, scroll_to_photo_path.as_ref());

                if let Some(album_list_response) = sidebar_response.album_list_response {
                    if let Some(album_id) = album_list_response.selected {
                        self.apply_album_filter(album_id);
                    }

                    if let Some(double_clicked_path) = album_list_response.double_clicked_photo {
                        self.open_photo(&double_clicked_path);
                    }
                }

                if let Some(file_tree_response) = sidebar_response.file_tree_response {
                    // Handle double-clicks in the file tree
                    if let Some(double_clicked_path) = file_tree_response.double_clicked {
                        // When an image file is double-clicked, open it in the viewer
                        self.open_photo(&double_clicked_path);
                    }

                    // Handle file removal from the file tree
                    if let Some(removed_path) = file_tree_response.removed {
                        self.remove_photo(&removed_path);
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
            GalleryScenePane::Sidebar => "File Tree".into(),
        }
    }
}
