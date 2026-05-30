use std::path::PathBuf;

use egui::Ui;

use crate::{
    modal::{manager::ModalManager, new_album::NewAlbumModal},
    widget::{
        album_list::{AlbumList, AlbumListResponse, AlbumListState},
        book_list::{BookList, BookListEntry, BookListResponse, BookListState},
        file_tree::{FileTree, FileTreeResponse, FileTreeState},
        sectioned_sidebar::{
            MIN_EXPANDED_SECTION_HEIGHT, SectionedSidebarBuilder, section::CollapsableSectionState,
        },
    },
};

#[derive(Debug, Clone)]
pub struct LeftSidebarState {
    pub book_list_state: BookListState,
    pub file_tree_state: FileTreeState,
    pub album_list_state: AlbumListState,
    book_list_section_state: CollapsableSectionState,
    file_tree_section_state: CollapsableSectionState,
    album_list_section_state: CollapsableSectionState,
}

pub struct LeftSidebar<'a> {
    state: &'a mut LeftSidebarState,
}

#[derive(Debug, Clone)]
pub struct LeftSidebarResponse {
    pub book_list_response: Option<BookListResponse>,
    pub file_tree_response: Option<FileTreeResponse>,
    pub album_list_response: Option<AlbumListResponse>,
    pub create_book: bool,
}

impl Default for LeftSidebarState {
    fn default() -> Self {
        Self {
            book_list_state: BookListState::default(),
            file_tree_state: FileTreeState::default(),
            album_list_state: AlbumListState::default(),
            book_list_section_state: CollapsableSectionState::new(true, "Books".to_string())
                .with_default_expanded_height(MIN_EXPANDED_SECTION_HEIGHT),
            file_tree_section_state: CollapsableSectionState::new(true, "File Tree".to_string()),
            album_list_section_state: CollapsableSectionState::new(true, "Albums".to_string())
                .with_default_expanded_height(MIN_EXPANDED_SECTION_HEIGHT),
        }
    }
}

impl<'a> LeftSidebar<'a> {
    pub fn new(state: &'a mut LeftSidebarState) -> Self {
        Self { state }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        scroll_to_path: Option<&PathBuf>,
        books: &[BookListEntry],
        selected_book_id: Option<&str>,
    ) -> LeftSidebarResponse {
        let mut book_list_response = None;
        let mut file_tree_response: Option<FileTreeResponse> = None;
        let mut album_list_response = None;
        let mut create_book = false;
        SectionedSidebarBuilder::new("collection_left_sidebar")
            .section(ui, &mut self.state.file_tree_section_state, |ui| {
                file_tree_response =
                    Some(FileTree::new(&mut self.state.file_tree_state).show(ui, scroll_to_path));
            })
            .section_with_action(
                ui,
                &mut self.state.album_list_section_state,
                "+",
                "New album",
                || {
                    ModalManager::push(NewAlbumModal::new());
                },
                |ui| {
                    album_list_response = Some(
                        AlbumList::new(&mut self.state.album_list_state).show(ui, scroll_to_path),
                    );
                },
            )
            .section_with_action(
                ui,
                &mut self.state.book_list_section_state,
                "+",
                "New book",
                || {
                    create_book = true;
                },
                |ui| {
                    book_list_response = Some(
                        BookList::new(&mut self.state.book_list_state, books, selected_book_id)
                            .show(ui),
                    );
                },
            )
            .show(ui);

        LeftSidebarResponse {
            book_list_response,
            file_tree_response,
            album_list_response,
            create_book,
        }
    }
}
