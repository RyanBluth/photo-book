use std::path::PathBuf;

use egui::Ui;

use crate::{
    modal::{manager::ModalManager, new_album::NewAlbumModal},
    widget::{
        album_list::{AlbumList, AlbumListResponse, AlbumListState},
        file_tree::{FileTree, FileTreeResponse, FileTreeState},
        sectioned_sidebar::{SectionedSidebarBuilder, section::CollapsableSectionState},
    },
};

#[derive(Debug, Clone)]
pub struct LeftSidebarState {
    pub file_tree_state: FileTreeState,
    pub album_list_state: AlbumListState,
    file_tree_section_state: CollapsableSectionState,
    album_list_section_state: CollapsableSectionState,
}

pub struct LeftSidebar<'a> {
    state: &'a mut LeftSidebarState,
}

#[derive(Debug, Clone)]
pub struct LeftSidebarResponse {
    pub file_tree_response: Option<FileTreeResponse>,
    pub album_list_response: Option<AlbumListResponse>,
}

impl Default for LeftSidebarState {
    fn default() -> Self {
        Self {
            file_tree_state: FileTreeState::default(),
            album_list_state: AlbumListState::default(),
            file_tree_section_state: CollapsableSectionState::new(false, "File Tree".to_string()),
            album_list_section_state: CollapsableSectionState::new(false, "Albums".to_string()),
        }
    }
}

impl<'a> LeftSidebar<'a> {
    pub fn new(state: &'a mut LeftSidebarState) -> Self {
        Self { state }
    }

    pub fn show(&mut self, ui: &mut Ui, scroll_to_path: Option<&PathBuf>) -> LeftSidebarResponse {
        let mut file_tree_response: Option<FileTreeResponse> = None;
        let mut album_list_response = None;
        SectionedSidebarBuilder::new("left_sidebar")
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
            .show(ui);

        LeftSidebarResponse {
            file_tree_response: file_tree_response,
            album_list_response: album_list_response,
        }
    }
}
