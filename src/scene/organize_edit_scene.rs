use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use egui::{Context, Id, Margin, Ui};
#[cfg(not(target_os = "macos"))]
use egui::{Pos2, Rect};
use egui_tiles::Tree;
use log::{error, info};

use crate::{
    auto_persisting::AutoPersisting,
    config::Config,
    debug::DebugSettings,
    dep, dep_mut,
    export::Exporter,
    file_dialog,
    modal::{
        ModalActionResponse,
        basic::BasicModal,
        file_import::FileImportModal,
        manager::{ModalManager, TypedModalId},
        name_prompt::{NamePromptModal, NamePromptResponse},
        page_settings::PageSettingsModal,
    },
    model::photo_grouping::PhotoGrouping,
    photo_manager::PhotoManager,
    project::ProjectPreferences,
    project_settings::ProjectSettingsManager,
    selection_manager::SelectionManager,
    session::{Session, SessionError},
    string_log::StringLog,
    theme::color,
    utils::Toggle,
    widget::{
        book_list::BookListEntry,
        left_sidebar::{LeftSidebar, LeftSidebarResponse, LeftSidebarState},
        log_viewer::LogViewer,
        status_bar::StatusBar,
    },
};

use super::{
    Scene, ScenePopResponse, SceneResponse,
    SceneTransition::{self},
    canvas_scene::{CanvasScene, CanvasSceneState},
    gallery_scene::GalleryScene,
    viewer_scene::ViewerScene,
};

mod book;
mod photo_viewer;
mod workspace;

use book::OpenBookEditor;
pub use book::{Book, BookId};
use photo_viewer::OpenPhotoViewer;
use workspace::{WorkspaceAction, WorkspacePane};

#[derive(Debug, Clone)]
pub struct OrganizeEditScene {
    pub organize: Arc<RwLock<GalleryScene>>,
    pub books: Vec<Book>,
    open_books: Vec<OpenBookEditor>,
    open_photo_viewers: Vec<OpenPhotoViewer>,
    workspace_tabs: Tree<WorkspacePane>,
    selected_book_id: Option<BookId>,
    left_sidebar_state: LeftSidebarState,
    page_settings_modal_id: Option<TypedModalId<PageSettingsModal>>,
    new_book_modal_id: Option<TypedModalId<NamePromptModal>>,
    pending_new_book_name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum MenuCommand {
    NewCollection,
    OpenCollection,
    OpenRecent(PathBuf),
    Save,
    AddToCollection,
    ImportFiles,
    Export,
    GroupByDate,
    GroupByRating,
    PageSettings,
    ToggleQuickLayoutNumbers,
}

impl OrganizeEditScene {
    pub fn new(organize: GalleryScene, edit: Option<CanvasScene>) -> Self {
        let books = edit
            .map(|edit| {
                Book::with_state(
                    uuid::Uuid::new_v4().to_string(),
                    "Book 1".to_string(),
                    edit.state,
                )
            })
            .into_iter()
            .collect();

        Self::with_books(organize, books)
    }

    pub fn with_books(organize: GalleryScene, books: Vec<Book>) -> Self {
        let organize_scene: Arc<RwLock<GalleryScene>> = Arc::new(RwLock::new(organize));
        Self {
            organize: organize_scene.clone(),
            books,
            open_books: Vec::new(),
            open_photo_viewers: Vec::new(),
            workspace_tabs: Self::initial_workspace_tabs(),
            selected_book_id: None,
            left_sidebar_state: LeftSidebarState::default(),
            page_settings_modal_id: None,
            new_book_modal_id: None,
            pending_new_book_name: None,
        }
    }

    pub fn show_gallery(&mut self) {
        self.persist_active_book_state();
        self.sync_organize_gallery_from_edit();
        self.activate_gallery_tab();
        self.selected_book_id = None;
    }

    pub fn books_snapshot(&self) -> Vec<Book> {
        let mut books = self.books.clone();

        for open_book in &self.open_books {
            if let Some(book) = books
                .iter_mut()
                .find(|book| book.id.as_str() == open_book.book_id.as_str())
            {
                book.state = open_book.scene.read().unwrap().state.clone();
            }
        }

        books
    }

    pub fn has_books(&self) -> bool {
        !self.books_snapshot().is_empty()
    }

    fn request_create_book(&mut self) {
        if self.new_book_modal_id.is_some() {
            return;
        }

        self.new_book_modal_id = Some(ModalManager::push(NamePromptModal::new(
            "New Book",
            "Book name",
            "Create",
        )));
    }

    fn create_book(&mut self, name: String) {
        self.persist_active_book_state();

        let mut book = Book::new(name);
        book.state.gallery_state = self
            .organize
            .read()
            .unwrap()
            .state
            .image_gallery_state
            .clone();

        let book_id = book.id.clone();
        self.books.push(book);
        self.select_book(&book_id);
    }

    fn create_book_or_request_page_settings(&mut self, name: String) {
        if dep!(ProjectSettingsManager, |project_settings_manager| {
            project_settings_manager
                .project_settings
                .default_page
                .is_none()
        }) {
            self.pending_new_book_name = Some(name);
            if self.page_settings_modal_id.is_none() {
                self.page_settings_modal_id = Some(ModalManager::push(PageSettingsModal::new()));
            }
            return;
        }

        self.create_book(name);
    }

    fn canvas_scene_for_book(book_id: &str, state: CanvasSceneState) -> CanvasScene {
        CanvasScene::with_state_and_tree_id(state, Id::new("canvas_scene_tree").with(book_id))
    }

    pub(in crate::scene::organize_edit_scene) fn select_book(&mut self, book_id: &str) {
        if self.selected_book_id.as_deref() == Some(book_id)
            && self.open_books.iter().any(|open| open.book_id == book_id)
        {
            self.activate_book_tab(book_id);
            return;
        }

        self.persist_active_book_state();

        if let Some(open_book_id) = self
            .open_books
            .iter()
            .find(|open_book| open_book.book_id.as_str() == book_id)
            .map(|open_book| open_book.book_id.clone())
        {
            self.activate_book_tab(book_id);
            self.selected_book_id = Some(open_book_id);
            return;
        }

        let Some(book) = self.books.iter().find(|book| book.id == book_id).cloned() else {
            return;
        };

        let edit_scene = Arc::new(RwLock::new(Self::canvas_scene_for_book(
            book_id, book.state,
        )));
        self.apply_project_preferences_to_canvas_scene(&edit_scene);
        let tile_id = self.insert_book_tab(book_id.to_string());
        self.open_books.push(OpenBookEditor {
            book_id: book_id.to_string(),
            scene: edit_scene.clone(),
            tile_id,
        });
        self.selected_book_id = Some(book_id.to_string());
    }

    pub(in crate::scene::organize_edit_scene) fn open_photo_viewer(
        &mut self,
        mut scene: ViewerScene,
    ) {
        let preferences = Self::current_project_preferences();
        scene.set_right_sidebar_open(preferences.right_sidebar_open);
        let photo_path = scene.photo_path().clone();

        if self.selected_book_id.is_some() {
            self.persist_active_book_state();
        }

        if let Some(viewer_id) = self
            .open_photo_viewers
            .iter()
            .find(|open_viewer| open_viewer.scene.read().unwrap().photo_path() == &photo_path)
            .map(|open_viewer| open_viewer.viewer_id.clone())
        {
            self.activate_photo_viewer_tab(&viewer_id);
            self.selected_book_id = None;
            return;
        }

        let viewer_id = uuid::Uuid::new_v4().to_string();
        let viewer_scene = Arc::new(RwLock::new(scene));
        let tile_id = self.insert_photo_viewer_tab(viewer_id.clone());
        self.open_photo_viewers.push(OpenPhotoViewer {
            viewer_id,
            scene: viewer_scene,
            tile_id,
        });
        self.selected_book_id = None;
    }

    pub(in crate::scene::organize_edit_scene) fn persist_active_book_state(&mut self) {
        let open_book_ids = self
            .open_books
            .iter()
            .map(|open_book| open_book.book_id.clone())
            .collect::<Vec<_>>();

        for book_id in open_book_ids {
            self.persist_book_state(&book_id);
        }

        if let Some(selected_book_id) = self.selected_book_id.clone() {
            self.persist_book_state(&selected_book_id);
        }
    }

    pub(in crate::scene::organize_edit_scene) fn persist_book_state(&mut self, book_id: &str) {
        let state = self
            .open_books
            .iter()
            .find(|open_book| open_book.book_id.as_str() == book_id)
            .map(|open_book| open_book.scene.read().unwrap().state.clone());

        let Some(state) = state else {
            return;
        };

        if let Some(book) = self
            .books
            .iter_mut()
            .find(|book| book.id.as_str() == book_id)
        {
            book.state = state;
        }
    }

    pub(in crate::scene::organize_edit_scene) fn sync_organize_gallery_from_edit(&mut self) {
        if let Some(book_state) = self.selected_book_state() {
            self.organize.write().unwrap().state.image_gallery_state = book_state.gallery_state;
        }
    }

    fn book_list_entries(&self) -> Vec<BookListEntry> {
        let mut entries = self.books.iter().map(Book::list_entry).collect::<Vec<_>>();

        for open_book in &self.open_books {
            if let Some(entry) = entries
                .iter_mut()
                .find(|entry| entry.id.as_str() == open_book.book_id.as_str())
            {
                entry.page_count = open_book
                    .scene
                    .read()
                    .unwrap()
                    .state
                    .pages_state
                    .pages
                    .len();
            }
        }

        entries
    }

    fn selected_book_state(&self) -> Option<CanvasSceneState> {
        if let Some(selected_book_id) = &self.selected_book_id
            && let Some(open_book) = self
                .open_books
                .iter()
                .find(|open_book| open_book.book_id.as_str() == selected_book_id.as_str())
        {
            return Some(open_book.scene.read().unwrap().state.clone());
        }

        let selected_book_id = self.selected_book_id.as_ref()?;
        self.books
            .iter()
            .find(|book| book.id.as_str() == selected_book_id.as_str())
            .map(|book| book.state.clone())
    }

    fn export_book_state(&self) -> Option<CanvasSceneState> {
        self.selected_book_state().or_else(|| {
            let books = self.books_snapshot();
            if books.len() == 1 {
                books.first().map(|book| book.state.clone())
            } else {
                None
            }
        })
    }

    fn scroll_to_photo_path(&mut self) -> Option<PathBuf> {
        let selection_change = dep!(SelectionManager, |selection_manager| {
            selection_manager.last_frame_selection()
        })?;

        for path in &selection_change.added_paths {
            self.left_sidebar_state
                .file_tree_state
                .expand_parent_directories(path);
        }

        selection_change.added_paths.first().cloned()
    }

    fn clear_photo_selection(&mut self) {
        dep_mut!(SelectionManager, |selection_manager| selection_manager
            .clear());
    }

    pub fn handle_menu_command(&mut self, command: MenuCommand, ctx: &Context) {
        match command {
            MenuCommand::NewCollection => {
                dep_mut!(Session, |session| {
                    match session.new_project(self) {
                        Ok(scene) => {
                            *self = scene;
                            self.show_gallery();
                        }
                        Err(SessionError::WaitingForUserInput) => {}
                        Err(e) => {
                            error!("Error creating new collection: {:?}", e);
                        }
                    }
                });
            }
            MenuCommand::OpenCollection => {
                self.load_collection(None, ctx);
            }
            MenuCommand::OpenRecent(path) => {
                self.load_collection(Some(path), ctx);
            }
            MenuCommand::Save => {
                self.persist_active_book_state();
                if let Err(err) = dep_mut!(Session, |session| session.save_project(self, ctx))
                    && !matches!(err, SessionError::WaitingForUserInput)
                {
                    error!("Error saving collection: {:?}", err);
                }
            }
            MenuCommand::AddToCollection => {
                self.import_photos(ctx);
            }
            MenuCommand::ImportFiles => {
                ModalManager::push(FileImportModal::new());
            }
            MenuCommand::Export => {
                self.export_selected_book(ctx);
            }
            MenuCommand::GroupByDate => {
                dep_mut!(PhotoManager, |photo_manager| {
                    photo_manager.group_photos_by(PhotoGrouping::Date);
                });
            }
            MenuCommand::GroupByRating => {
                dep_mut!(PhotoManager, |photo_manager| {
                    photo_manager.group_photos_by(PhotoGrouping::Rating);
                });
            }
            MenuCommand::PageSettings => {
                self.page_settings_modal_id = Some(ModalManager::push(PageSettingsModal::new()));
            }
            MenuCommand::ToggleQuickLayoutNumbers => {
                dep_mut!(DebugSettings, |debug_settings| {
                    debug_settings.show_quick_layout_order.toggle();
                });
            }
        }
    }

    fn load_collection(&mut self, path: Option<PathBuf>, ctx: &Context) {
        match dep_mut!(Session, |session| session.load_project(self, path, ctx)) {
            Ok(scene) => {
                *self = scene;
                self.show_gallery();
            }
            Err(SessionError::WaitingForUserInput) => {}
            Err(err) => {
                error!("Error loading collection: {:?}", err);

                ModalManager::push(BasicModal::new(
                    "Error",
                    format!("Error loading collection: {:?}", err),
                    "OK",
                ));
            }
        }
    }

    fn import_photos(&mut self, ctx: &Context) {
        let dialog = native_dialog::DialogBuilder::file()
            .add_filter("Images", ["png", "jpg", "jpeg"])
            .open_single_dir()
            .spawn();

        file_dialog::spawn(dialog, ctx.clone(), |result, _| match result {
            Ok(Some(import_dir)) => {
                info!("Importing {import_dir:?}");
                if let Err(error) = PhotoManager::load_directory(import_dir) {
                    error!("Error importing photos: {error:?}");
                }
            }
            Err(error) => error!("Error opening import file dialog: {error}"),
            Ok(None) => info!("No import directory selected"),
        });
    }

    fn export_selected_book(&mut self, ctx: &Context) {
        self.persist_active_book_state();
        let Some(book_state) = self.export_book_state() else {
            ModalManager::push(BasicModal::new("Error", "Select a book to export", "OK"));
            return;
        };
        let pages = book_state.pages_state.pages.values().cloned().collect();

        let dialog = native_dialog::DialogBuilder::file()
            .set_filename("export.pdf")
            .save_single_file()
            .spawn();
        file_dialog::spawn(dialog, ctx.clone(), move |result, ctx| match result {
            Ok(Some(export_path)) => {
                let directory = export_path.parent().unwrap();
                let file_name = export_path.file_name().unwrap();

                dep_mut!(Exporter, |exporter| {
                    exporter.export(
                        ctx.clone(),
                        pages,
                        directory.into(),
                        file_name.to_str().unwrap(),
                    );
                });
            }
            Err(error) => error!("Error opening export file dialog: {error}"),
            Ok(None) => info!("No export directory selected"),
        });
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

    fn open_photo_response(&self, path: &PathBuf) -> Option<SceneResponse> {
        dep!(PhotoManager, |photo_manager| {
            photo_manager
                .photo_database
                .get_photo(path)
                .cloned()
                .map(|photo| SceneResponse::Push(SceneTransition::Viewer(ViewerScene::new(photo))))
        })
    }

    fn handle_left_sidebar_response(
        &mut self,
        sidebar_response: LeftSidebarResponse,
    ) -> (Option<SceneResponse>, Option<WorkspaceAction>) {
        let mut workspace_action = None;

        if sidebar_response.create_book {
            workspace_action = Some(WorkspaceAction::CreateBook);
        }

        if let Some(book_list_response) = sidebar_response.book_list_response
            && let Some(book_id) = book_list_response.selected
        {
            workspace_action = Some(WorkspaceAction::SelectBook(book_id));
        }

        if let Some(album_list_response) = sidebar_response.album_list_response {
            if let Some(album_id) = album_list_response.selected {
                self.apply_album_filter(album_id);
            }

            if let Some(double_clicked_path) = album_list_response.double_clicked_photo {
                return (
                    self.open_photo_response(&double_clicked_path),
                    workspace_action,
                );
            }
        }

        if let Some(file_tree_response) = sidebar_response.file_tree_response {
            if let Some(double_clicked_path) = file_tree_response.double_clicked {
                return (
                    self.open_photo_response(&double_clicked_path),
                    workspace_action,
                );
            }

            if let Some(removed_path) = file_tree_response.removed {
                self.remove_photo(&removed_path);
            }
        }

        (None, workspace_action)
    }

    pub(in crate::scene::organize_edit_scene) fn apply_workspace_action(
        &mut self,
        action: WorkspaceAction,
    ) {
        match action {
            WorkspaceAction::CreateBook => self.request_create_book(),
            WorkspaceAction::SelectBook(book_id) => self.select_book(&book_id),
        }
    }

    fn check_new_book_modal(&mut self) {
        let Some(id) = self.new_book_modal_id else {
            return;
        };

        let exists = dep!(ModalManager, |modal_manager| modal_manager.exists(id));

        let modal_response = dep!(ModalManager, |modal_manager| modal_manager
            .response_for(&id));
        if let Ok(Some(NamePromptResponse::Confirm { name })) = modal_response {
            self.create_book_or_request_page_settings(name);
        }

        if !exists {
            self.new_book_modal_id = None;
        }
    }

    fn check_page_settings_modal(&mut self) {
        let Some(id) = self.page_settings_modal_id else {
            return;
        };

        let exists = dep!(ModalManager, |modal_manager| modal_manager.exists(id));

        let modal_response = dep!(ModalManager, |modal_manager| modal_manager
            .response_for(&id));
        if let Ok(Some(ModalActionResponse::Confirm)) = modal_response {
            if let Some(name) = self.pending_new_book_name.take() {
                self.create_book(name);
            } else {
                self.persist_active_book_state();
            }
        }

        if !exists {
            self.page_settings_modal_id = None;
            self.pending_new_book_name = None;
        }
    }

    #[allow(dead_code)]
    fn menu_bar(&mut self, ui: &mut Ui) {
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("New Collection").clicked() {
                            self.handle_menu_command(MenuCommand::NewCollection, ui.ctx());
                        }

                        if ui.button("Open Collection").clicked() {
                            self.handle_menu_command(MenuCommand::OpenCollection, ui.ctx());
                        }

                        ui.menu_button("Open Recent", |ui| {
                            let recents = dep_mut!(AutoPersisting<Config>, |config| {
                                config.read().unwrap().recent_projects().to_vec()
                            });

                            if recents.is_empty() {
                                ui.label("No recent collections");
                            } else {
                                for recent in &recents {
                                    if ui.button(recent.display().to_string()).clicked() {
                                        self.handle_menu_command(
                                            MenuCommand::OpenRecent(recent.clone()),
                                            ui.ctx(),
                                        );
                                    }
                                }
                            }
                        });

                        if ui.button("Save").clicked() {
                            self.handle_menu_command(MenuCommand::Save, ui.ctx());
                        }

                        if ui.button("Add to Collection").clicked() {
                            self.handle_menu_command(MenuCommand::AddToCollection, ui.ctx());
                        }

                        if ui.button("Import Files").clicked() {
                            self.handle_menu_command(MenuCommand::ImportFiles, ui.ctx());
                        }

                        if ui.button("Export").clicked() {
                            self.handle_menu_command(MenuCommand::Export, ui.ctx());
                        }
                    });

                    ui.menu_button("Group By", |ui| {
                        if ui.button("Date").clicked() {
                            self.handle_menu_command(MenuCommand::GroupByDate, ui.ctx());
                        }
                        if ui.button("Rating").clicked() {
                            self.handle_menu_command(MenuCommand::GroupByRating, ui.ctx());
                        }
                    });

                    ui.menu_button("Collection Settings", |ui| {
                        if ui.button("Page Settings").clicked() {
                            self.handle_menu_command(MenuCommand::PageSettings, ui.ctx());
                        }
                    });

                    ui.menu_button("Debug", |ui| {
                        fn enabled_disabled_suffix(enabled: bool) -> &'static str {
                            if enabled { "(Enabled)" } else { "(Disabled)" }
                        }

                        let show_quick_layout_order = dep!(DebugSettings, |debug_settings| {
                            debug_settings.show_quick_layout_order
                        });

                        if ui
                            .button(format!(
                                "Quick Layout Numbers:{}",
                                enabled_disabled_suffix(show_quick_layout_order)
                            ))
                            .clicked()
                        {
                            self.handle_menu_command(
                                MenuCommand::ToggleQuickLayoutNumbers,
                                ui.ctx(),
                            );
                        }
                    })
                });
            });
    }

    pub(in crate::scene::organize_edit_scene) fn collection_mode_ui(
        &mut self,
        ui: &mut Ui,
    ) -> (SceneResponse, Option<WorkspaceAction>) {
        let scroll_to_photo_path = self.scroll_to_photo_path();
        let book_entries = self.book_list_entries();
        let selected_book_id = self.selected_book_id.clone();
        let mut sidebar_response = None;

        if Self::current_project_preferences().left_sidebar_open {
            egui::Panel::left("collection_gallery_sidebar")
                .resizable(true)
                .default_size(300.0)
                .size_range(240.0..=480.0)
                .frame(egui::Frame {
                    inner_margin: Margin::ZERO,
                    fill: color::SIDE_PANEL_BACKGROUND,
                    ..Default::default()
                })
                .show(ui, |ui| {
                    sidebar_response = Some(LeftSidebar::new(&mut self.left_sidebar_state).show(
                        ui,
                        scroll_to_photo_path.as_ref(),
                        &book_entries,
                        selected_book_id.as_deref(),
                    ));
                });
        }

        let content_response = egui::CentralPanel::default()
            .frame(egui::Frame {
                inner_margin: Margin::ZERO,
                fill: color::SURFACE_X_DARK,
                ..Default::default()
            })
            .show(ui, |ui| self.organize.write().unwrap().ui(ui))
            .inner;

        let (pending_response, workspace_action) = match sidebar_response {
            Some(response) => self.handle_left_sidebar_response(response),
            None => (None, None),
        };

        let scene_response = pending_response.unwrap_or(content_response);

        (scene_response, workspace_action)
    }

    pub(in crate::scene::organize_edit_scene) fn handle_child_scene_response(
        &mut self,
        scene_response: SceneResponse,
    ) -> SceneResponse {
        match scene_response {
            SceneResponse::Push(transition) => match transition {
                SceneTransition::Gallery(scene) => {
                    *self.organize.write().unwrap() = scene;
                    self.show_gallery();
                    self.apply_session_project_preferences();
                    SceneResponse::None
                }
                SceneTransition::Canvas(scene) => {
                    let state = scene.state;
                    let book_id = if let Some(selected_book_id) = self.selected_book_id.clone()
                        && let Some(book) = self
                            .books
                            .iter_mut()
                            .find(|book| book.id.as_str() == selected_book_id.as_str())
                    {
                        book.state = state.clone();
                        selected_book_id
                    } else {
                        let book_id = uuid::Uuid::new_v4().to_string();
                        self.books.push(Book::with_state(
                            book_id.clone(),
                            format!("Book {}", self.books.len() + 1),
                            state.clone(),
                        ));
                        book_id
                    };

                    if let Some(open_book) = self
                        .open_books
                        .iter_mut()
                        .find(|open_book| open_book.book_id == book_id)
                    {
                        let mut open_scene = open_book.scene.write().unwrap();
                        open_scene.state = state;
                        let preferences = Self::current_project_preferences();
                        open_scene.set_right_sidebar_open(preferences.right_sidebar_open);
                        open_scene.set_left_sidebar_open(preferences.left_sidebar_open);
                    } else {
                        let edit_scene =
                            Arc::new(RwLock::new(Self::canvas_scene_for_book(&book_id, state)));
                        self.apply_project_preferences_to_canvas_scene(&edit_scene);
                        let tile_id = self.insert_book_tab(book_id.clone());
                        self.open_books.push(OpenBookEditor {
                            book_id: book_id.clone(),
                            scene: edit_scene.clone(),
                            tile_id,
                        });
                    }

                    self.activate_book_tab(&book_id);
                    self.selected_book_id = Some(book_id);
                    SceneResponse::None
                }
                SceneTransition::Viewer(scene) => {
                    self.open_photo_viewer(scene);
                    SceneResponse::None
                }
                _ => SceneResponse::Push(transition),
            },
            _ => scene_response,
        }
    }

    pub fn apply_project_preferences(&mut self, preferences: &ProjectPreferences) {
        self.organize
            .write()
            .unwrap()
            .set_right_sidebar_open(preferences.right_sidebar_open);

        for open_book in &self.open_books {
            let mut scene = open_book.scene.write().unwrap();
            scene.set_right_sidebar_open(preferences.right_sidebar_open);
            scene.set_left_sidebar_open(preferences.left_sidebar_open);
        }

        for open_viewer in &self.open_photo_viewers {
            open_viewer
                .scene
                .write()
                .unwrap()
                .set_right_sidebar_open(preferences.right_sidebar_open);
        }
    }

    fn apply_session_project_preferences(&mut self) {
        let preferences = Self::current_project_preferences();
        self.apply_project_preferences(&preferences);
    }

    fn apply_project_preferences_to_canvas_scene(&self, scene: &Arc<RwLock<CanvasScene>>) {
        let preferences = Self::current_project_preferences();
        let mut scene = scene.write().unwrap();
        scene.set_right_sidebar_open(preferences.right_sidebar_open);
        scene.set_left_sidebar_open(preferences.left_sidebar_open);
    }

    fn current_project_preferences() -> ProjectPreferences {
        dep!(Session, |session| session.project_preferences.clone())
    }
}

impl Scene for OrganizeEditScene {
    fn ui(&mut self, ui: &mut Ui) -> SceneResponse {
        self.check_new_book_modal();
        self.check_page_settings_modal();
        let frame = egui::Frame {
            inner_margin: Margin::ZERO,
            fill: color::SURFACE_X_DARK,
            ..Default::default()
        };

        let status_bar_response = egui::Panel::bottom("root_status_bar")
            .exact_size(StatusBar::height())
            .frame(frame)
            .show(ui, |ui| StatusBar::new().show(ui))
            .inner;

        if status_bar_response.left_sidebar_toggled || status_bar_response.right_sidebar_toggled {
            self.apply_session_project_preferences();
        }

        if Self::current_project_preferences().log_viewer_open {
            egui::Panel::bottom("root_log_viewer")
                .resizable(true)
                .default_size(220.0)
                .size_range(120.0..=500.0)
                .frame(frame)
                .show(ui, |ui| {
                    dep!(StringLog, |log| {
                        LogViewer::new(log)
                            .id_salt("root_log_viewer_scroll")
                            .show(ui);
                    });
                });
        }

        egui::CentralPanel::default()
            .frame(frame)
            .show(ui, |ui| {
                #[cfg(not(target_os = "macos"))]
                {
                    ui.painter().rect_filled(
                        Rect::from_min_max(
                            Pos2::ZERO,
                            Pos2::new(ui.max_rect().width() + 100.0, 34.0),
                        ),
                        0.0,
                        color::SURFACE,
                    );
                }

                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;

                    #[cfg(not(target_os = "macos"))]
                    {
                        self.menu_bar(ui);
                        ui.add_space(8.0);
                    }

                    self.workspace_tabs_ui(ui)
                })
                .inner
            })
            .inner
    }

    fn popped(&mut self, popped_scene_response: ScenePopResponse) {
        if let Some(selected_book_id) = &self.selected_book_id
            && let Some(open_book) = self
                .open_books
                .iter()
                .find(|open_book| open_book.book_id.as_str() == selected_book_id.as_str())
        {
            open_book
                .scene
                .write()
                .unwrap()
                .popped(popped_scene_response);
        } else {
            self.organize.write().unwrap().popped(popped_scene_response);
        }
    }
}
