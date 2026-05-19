use std::path::PathBuf;

use egui_tiles::UiResponse;

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    photo::SaveOnDropPhoto,
    photo_manager::PhotoManager,
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

    // New fields for inter-widget communication - replacing static vars
    /// Path from gallery to scroll the file tree to
    pub scroll_file_tree_to_path: Option<PathBuf>,
    /// Path from file tree to scroll the gallery to
    pub scroll_gallery_to_path: Option<PathBuf>,
}

impl Default for GallerySceneState {
    fn default() -> Self {
        Self {
            image_gallery_state: ImageGalleryState::default(),
            photo_info_state: PhotoInfoState::new(),
            left_sidebar_state: LeftSidebarState::default(),

            // Initialize new fields
            scroll_file_tree_to_path: None,
            scroll_gallery_to_path: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GalleryScenePane {
    Gallery,
    PhotoInfo,
    FileTree,
}

#[derive(Debug, Clone)]
pub struct GalleryScene {
    pub state: GallerySceneState,
    tree: egui_tiles::Tree<GalleryScenePane>,
}

impl GalleryScene {
    pub fn new() -> Self {
        let mut tiles = egui_tiles::Tiles::default();

        let gallery_pane_id = tiles.insert_pane(GalleryScenePane::Gallery);

        let right_tabs = vec![tiles.insert_pane(GalleryScenePane::PhotoInfo)];
        let right_tabs_id = tiles.insert_tab_tile(right_tabs);

        let left_tabs = vec![tiles.insert_pane(GalleryScenePane::FileTree)];
        let left_tabs_id = tiles.insert_tab_tile(left_tabs);

        let mut linear_layout = egui_tiles::Linear::new(
            egui_tiles::LinearDir::Horizontal,
            vec![left_tabs_id, gallery_pane_id, right_tabs_id],
        );

        linear_layout.shares.set_share(right_tabs_id, 0.2);
        linear_layout.shares.set_share(left_tabs_id, 0.2);

        Self {
            state: GallerySceneState::default(),
            tree: egui_tiles::Tree::new(
                "organize_scene_tree",
                tiles.insert_container(linear_layout),
                tiles,
            ),
        }
    }
}

impl Scene for GalleryScene {
    fn ui(&mut self, ui: &mut egui::Ui) -> SceneResponse {
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
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        photo_manager.with_lock(|photo_manager| {
            if let Some(photo) = photo_manager.photo_database.get_photo(path) {
                self.navigator
                    .push(SceneTransition::Viewer(ViewerScene::new(photo.clone())));
            }
        });
    }

    fn select_photo_in_gallery(&mut self, path: &PathBuf) {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();

        photo_manager.with_lock(|photo_manager| {
            if let Some(photo) = photo_manager.photo_database.get_photo(path) {
                self.scene_state.image_gallery_state.selected_images.clear();
                self.scene_state
                    .image_gallery_state
                    .selected_images
                    .insert(photo.path.clone());
                self.scene_state.scroll_gallery_to_path = Some(photo.path.clone());
            }
        });
    }

    fn select_photo_in_file_tree(&mut self, path: PathBuf) {
        self.scene_state
            .left_sidebar_state
            .file_tree_state
            .selected_node = Some(path.clone());

        self.expand_file_tree_parent_directories(&path);
        self.scene_state.scroll_file_tree_to_path = Some(path);
    }

    fn expand_file_tree_parent_directories(&mut self, path: &PathBuf) {
        let mut current_path = path.clone();
        while let Some(parent) = current_path.parent() {
            if parent.as_os_str().is_empty() {
                break;
            }

            self.scene_state
                .left_sidebar_state
                .file_tree_state
                .expanded_directories
                .insert(parent.to_path_buf());
            current_path = parent.to_path_buf();
        }
    }

    fn apply_album_filter(&mut self, album_id: String) {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        photo_manager.with_lock_mut(|photo_manager| {
            let mut filter = photo_manager.get_current_filter().clone();
            filter.album = Some(album_id);
            photo_manager.set_current_filter(filter);
        });

        self.scene_state.image_gallery_state.selected_images.clear();
        self.scene_state
            .left_sidebar_state
            .file_tree_state
            .selected_node = None;
    }

    fn remove_photo(&mut self, path: &PathBuf) {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        photo_manager.with_lock_mut(|photo_manager| {
            photo_manager.photo_database.remove_photo(path);
            self.scene_state
                .image_gallery_state
                .selected_images
                .remove(path);
        });
    }

    fn ui(&mut self, pane: &GalleryScenePane, ui: &mut egui::Ui) -> UiResponse {
        match pane {
            GalleryScenePane::Gallery => {
                // Show the gallery with any scroll-to info from the state
                let gallery_response = ImageGallery::show(
                    ui,
                    &mut self.scene_state.image_gallery_state,
                    self.scene_state.scroll_gallery_to_path.as_ref(),
                );

                // Clear scroll data after it's been used
                self.scene_state.scroll_gallery_to_path = None;

                // Handle gallery responses
                if let Some(photo) = gallery_response.primary_action_photo {
                    // Handle primary action (double-click) - open viewer
                    self.navigator
                        .push(SceneTransition::Viewer(ViewerScene::new(photo)));
                }

                // Handle selection changes in gallery for file tree synchronization
                if let Some(selected_photo) = gallery_response.selected_photo {
                    self.select_photo_in_file_tree(selected_photo.path);
                }

                // Handle selection clearing
                if gallery_response.selection_cleared {
                    self.scene_state
                        .left_sidebar_state
                        .file_tree_state
                        .selected_node = None;
                }

                UiResponse::None
            }
            GalleryScenePane::PhotoInfo => {
                let photo_manager: Singleton<PhotoManager> = Dependency::get();
                let gallery_state = &self.scene_state.image_gallery_state;

                match gallery_state.selected_images.iter().next() {
                    Some(selected_image) => {
                        let mut photo = photo_manager.with_lock(|photo_manager| {
                            photo_manager
                                .photo_database
                                .get_photo(selected_image)
                                .unwrap()
                                .clone()
                        });

                        PhotoInfo::new(
                            SaveOnDropPhoto::new(&mut photo),
                            &mut self.scene_state.photo_info_state,
                        )
                        .show(ui);
                    }
                    _ => {
                        ui.both_centered(|ui| {
                            ui.heading("Nothing selected");
                        });
                    }
                }

                UiResponse::None
            }
            GalleryScenePane::FileTree => {
                let sidebar_response = LeftSidebar::new(&mut self.scene_state.left_sidebar_state)
                    .show(ui, self.scene_state.scroll_file_tree_to_path.as_ref());

                if let Some(album_list_response) = sidebar_response.album_list_response {
                    if let Some(album_id) = album_list_response.selected {
                        self.apply_album_filter(album_id);
                    }

                    if let Some(selected_path) = album_list_response.selected_photo {
                        self.select_photo_in_gallery(&selected_path);
                    }

                    if let Some(double_clicked_path) = album_list_response.double_clicked_photo {
                        self.open_photo(&double_clicked_path);
                    }
                }

                if let Some(file_tree_response) = sidebar_response.file_tree_response {
                    // Clear the scroll target after it's been used
                    self.scene_state.scroll_file_tree_to_path = None;

                    // Handle file tree responses
                    if let Some(selected_path) = file_tree_response.selected {
                        // When a file is selected in the tree, update the gallery selection
                        self.select_photo_in_gallery(&selected_path);
                    }

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

impl egui_tiles::Behavior<GalleryScenePane> for GalleryTreeBehavior<'_> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut GalleryScenePane,
    ) -> UiResponse {
        self.ui(pane, ui)
    }

    fn tab_title_for_pane(&mut self, pane: &GalleryScenePane) -> egui::widget_text::WidgetText {
        match pane {
            GalleryScenePane::Gallery => "Gallery".into(),
            GalleryScenePane::PhotoInfo => "Photo Info".into(),
            GalleryScenePane::FileTree => "File Tree".into(),
        }
    }
}
