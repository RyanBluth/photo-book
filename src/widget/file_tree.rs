use std::collections::HashSet;
use std::path::PathBuf;

use egui::{Response, Sense, Ui};

use crate::{
    dependencies::{Dependency, SingletonFor},
    file_tree::{FileTreeNode, FlattenedTreeItem},
    photo_manager::PhotoManager,
    widget::tree_list::{INDENT_WIDTH, ROW_HEIGHT, SelectionStyle, TreeList, TreeListRow},
};

const BASE_WIDTH: f32 = 200.0;

#[derive(Debug, Clone)]
pub struct FileTreeState {
    pub expanded_directories: HashSet<PathBuf>,
    pub selected_node: Option<PathBuf>,
}

impl Default for FileTreeState {
    fn default() -> Self {
        Self {
            expanded_directories: HashSet::new(),
            selected_node: None,
        }
    }
}

pub struct FileTree<'a> {
    state: &'a mut FileTreeState,
}

#[derive(Debug, Clone)]
pub struct FileTreeResponse {
    pub _response: Response,

    pub selected: Option<PathBuf>,

    pub double_clicked: Option<PathBuf>,

    pub removed: Option<PathBuf>,
}

impl<'a> FileTree<'a> {
    pub fn new(state: &'a mut FileTreeState) -> Self {
        Self { state }
    }

    pub fn show(&mut self, ui: &mut Ui, scroll_to_path: Option<&PathBuf>) -> FileTreeResponse {
        ui.style_mut().interaction.selectable_labels = false;

        let mut selected_path_this_frame: Option<PathBuf> = None;
        let mut double_clicked_path_this_frame: Option<PathBuf> = None;
        let mut removed_path_this_frame: Option<PathBuf> = None;

        let items = Dependency::<PhotoManager>::get()
            .with_lock_mut(|pm| pm.photo_database.get_flattened_file_trees());
        let visible_items: Vec<&FlattenedTreeItem> = items
            .iter()
            .filter(|item| self.is_path_visible(item))
            .collect();
        let max_depth = visible_items
            .iter()
            .map(|item| item.depth)
            .max()
            .unwrap_or(0);

        let min_column_width = BASE_WIDTH + (max_depth as f32 * INDENT_WIDTH);

        let mut row_to_scroll: Option<usize> = None;
        if let Some(path_to_scroll) = scroll_to_path {
            for (idx, item) in visible_items.iter().enumerate() {
                match &item.node {
                    FileTreeNode::Directory(dir_path, _) => {
                        if dir_path.as_path() == path_to_scroll.as_path() {
                            row_to_scroll = Some(idx);
                            break;
                        }
                    }
                    FileTreeNode::File(file_path) => {
                        if file_path.as_path() == path_to_scroll.as_path() {
                            row_to_scroll = Some(idx);
                            break;
                        }
                    }
                }
            }
        }

        let mut disclosure_clicked_path: Option<PathBuf> = None;

        let outer_response = TreeList::new(ui)
            .id_salt("file_tree_scroll")
            .min_width(min_column_width)
            .scroll_to_row_top(row_to_scroll)
            .body(|body| {
                body.rows(ROW_HEIGHT, visible_items.len(), |mut row| {
                    let response = row.add(self.row_for_item(visible_items[row.index()]));

                    if response.disclosure_clicked() {
                        disclosure_clicked_path = Some(response.id().clone());
                    } else if response.clicked() {
                        let path = response.id().clone();
                        let new_selected = Some(path.clone());
                        if self.state.selected_node != new_selected {
                            self.state.selected_node = new_selected;
                            selected_path_this_frame = Some(path);
                        }
                    }

                    if response.double_clicked() {
                        double_clicked_path_this_frame = Some(response.id().clone());
                    }

                    let mut remove_clicked = false;
                    response.response().context_menu(|ui| {
                        if ui.button("Remove").clicked() {
                            remove_clicked = true;
                            ui.close();
                        }
                    });

                    if remove_clicked {
                        removed_path_this_frame = Some(response.id().clone());
                    }
                });
            });

        if let Some(path) = disclosure_clicked_path {
            if self.state.expanded_directories.contains(&path) {
                self.state.expanded_directories.remove(&path);
                self.collapse_all_subdirectories(&path);
            } else {
                self.state.expanded_directories.insert(path);
            }
        }

        FileTreeResponse {
            _response: outer_response,
            selected: selected_path_this_frame,
            double_clicked: double_clicked_path_this_frame,
            removed: removed_path_this_frame,
        }
    }

    fn row_for_item(&self, item: &FlattenedTreeItem) -> TreeListRow<PathBuf> {
        let item_path = item.node.path().clone();
        let title = if item.is_root {
            item.node.path().to_string_lossy().to_string()
        } else {
            item.node
                .path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        };
        let is_selected = self.state.selected_node.as_ref() == Some(&item_path);

        match &item.node {
            FileTreeNode::Directory(path, children) => TreeListRow::header(
                item_path,
                item.depth,
                title,
                !children.is_empty(),
                self.state.expanded_directories.contains(path),
            ),
            FileTreeNode::File(path) => {
                TreeListRow::photo(item_path, item.depth, title, path.clone())
            }
        }
        .selected(is_selected)
        .selection_style(SelectionStyle::Background)
        .sense(Sense::click_and_drag())
    }

    fn collapse_all_subdirectories(&mut self, path: &PathBuf) {
        let mut to_remove = Vec::new();

        for item in Dependency::<PhotoManager>::get()
            .with_lock_mut(|pm| pm.photo_database.get_flattened_file_trees())
            .iter()
        {
            match &item.node {
                FileTreeNode::Directory(dir_path, _) => {
                    if dir_path.starts_with(path) {
                        to_remove.push(dir_path.clone());
                    }
                }
                _ => {}
            }
        }

        for dir_path in to_remove {
            self.state.expanded_directories.remove(&dir_path);
        }
    }

    fn is_path_visible(&self, item: &FlattenedTreeItem) -> bool {
        if item.is_root {
            return true;
        }

        if self
            .state
            .expanded_directories
            .contains(item.node.path().parent().unwrap())
        {
            return true;
        }

        false
    }
}
