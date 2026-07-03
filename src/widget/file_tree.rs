use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use egui::{Sense, Ui};

use crate::{
    dep_mut,
    file_tree::{FileTreeNode, FlattenedTreeItem},
    photo_manager::PhotoManager,
    widget::tree_list::{
        SelectionStyle, TreeList, TreeListRow, TreeListRowResponse, TreeListSelection,
        INDENT_WIDTH, ROW_HEIGHT,
    },
};

const BASE_WIDTH: f32 = 200.0;

#[derive(Debug, Clone, Default)]
pub struct FileTreeState {
    expanded_directories: HashSet<PathBuf>,
}

impl FileTreeState {
    pub fn expand_parent_directories(&mut self, path: &Path) {
        let mut current_path = path.to_path_buf();
        while let Some(parent) = current_path.parent() {
            if parent.as_os_str().is_empty() {
                break;
            }

            self.expanded_directories.insert(parent.to_path_buf());
            current_path = parent.to_path_buf();
        }
    }

    fn is_expanded(&self, path: &Path) -> bool {
        self.expanded_directories.contains(path)
    }

    fn toggle_directory(&mut self, path: &Path) {
        if self.expanded_directories.remove(path) {
            self.collapse_directory(path);
        } else {
            self.expanded_directories.insert(path.to_path_buf());
        }
    }

    fn collapse_directory(&mut self, path: &Path) {
        self.expanded_directories
            .retain(|expanded_path| !expanded_path.starts_with(path));
    }

    fn is_path_visible(&self, item: &FlattenedTreeItem) -> bool {
        item.is_root
            || item
                .node
                .path()
                .parent()
                .is_some_and(|parent| self.is_expanded(parent))
    }
}

struct FileTreeLayout<'a> {
    visible_items: Vec<&'a FlattenedTreeItem>,
    ordered_photo_paths: Vec<PathBuf>,
    min_column_width: f32,
    row_to_scroll: Option<usize>,
}

impl<'a> FileTreeLayout<'a> {
    fn new(
        items: &'a [FlattenedTreeItem],
        state: &FileTreeState,
        scroll_to_path: Option<&PathBuf>,
    ) -> Self {
        let visible_items = items
            .iter()
            .filter(|item| state.is_path_visible(item))
            .collect::<Vec<_>>();
        let ordered_photo_paths = ordered_photo_paths(&visible_items);
        let min_column_width = min_column_width(&visible_items);
        let row_to_scroll = row_to_scroll(&visible_items, scroll_to_path);

        Self {
            visible_items,
            ordered_photo_paths,
            min_column_width,
            row_to_scroll,
        }
    }
}

pub struct FileTree<'a> {
    state: &'a mut FileTreeState,
}

#[derive(Debug, Clone)]
pub struct FileTreeResponse {
    pub double_clicked: Option<PathBuf>,

    pub removed: Option<PathBuf>,
}

impl<'a> FileTree<'a> {
    pub fn new(state: &'a mut FileTreeState) -> Self {
        Self { state }
    }

    pub fn show(&mut self, ui: &mut Ui, scroll_to_path: Option<&PathBuf>) -> FileTreeResponse {
        let mut selection = TreeListSelection::new(ui);
        let mut double_clicked: Option<PathBuf> = None;
        let mut removed: Option<PathBuf> = None;

        let items = dep_mut!(PhotoManager, |pm| pm
            .photo_database
            .get_flattened_file_trees());
        let layout = FileTreeLayout::new(&items, self.state, scroll_to_path);

        let mut disclosure_clicked_path: Option<PathBuf> = None;

        TreeList::new(ui)
            .id_salt("file_tree_scroll")
            .min_width(layout.min_column_width)
            .scroll_to_row_top(layout.row_to_scroll)
            .body(|body| {
                body.rows(ROW_HEIGHT, layout.visible_items.len(), |mut row| {
                    let response = row.add_selectable(
                        self.row_for_item(layout.visible_items[row.index()]),
                        &mut selection,
                        &layout.ordered_photo_paths,
                    );

                    if response.disclosure_clicked() {
                        disclosure_clicked_path = Some(response.id().clone());
                    }

                    if response.double_clicked() {
                        double_clicked = Some(response.id().clone());
                    }

                    if remove_requested(&response) {
                        removed = Some(response.id().clone());
                    }
                });
            });

        if let Some(path) = disclosure_clicked_path {
            self.state.toggle_directory(&path);
        }

        FileTreeResponse {
            double_clicked,
            removed,
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

        match &item.node {
            FileTreeNode::Directory(path, children) => TreeListRow::header(
                item_path,
                item.depth,
                title,
                !children.is_empty(),
                self.state.is_expanded(path),
            ),
            FileTreeNode::File(path) => {
                TreeListRow::photo(item_path, item.depth, title, path.clone())
            }
        }
        .selection_style(SelectionStyle::Background)
        .sense(Sense::click_and_drag())
    }
}

fn remove_requested(response: &TreeListRowResponse<PathBuf>) -> bool {
    let mut remove_clicked = false;
    response.response().context_menu(|ui| {
        if ui.button("Remove").clicked() {
            remove_clicked = true;
            ui.close();
        }
    });

    remove_clicked
}

fn ordered_photo_paths(items: &[&FlattenedTreeItem]) -> Vec<PathBuf> {
    // TODO: Avoid rebuilding all visible paths every frame; range selection only needs this on selection input.
    items
        .iter()
        .filter_map(|item| match &item.node {
            FileTreeNode::File(path) => Some(path.clone()),
            FileTreeNode::Directory(_, _) => None,
        })
        .collect()
}

fn min_column_width(items: &[&FlattenedTreeItem]) -> f32 {
    let max_depth = items.iter().map(|item| item.depth).max().unwrap_or(0);

    BASE_WIDTH + (max_depth as f32 * INDENT_WIDTH)
}

fn row_to_scroll(
    visible_items: &[&FlattenedTreeItem],
    scroll_to_path: Option<&PathBuf>,
) -> Option<usize> {
    let scroll_to_path = scroll_to_path?;

    visible_items
        .iter()
        .position(|item| item.node.path().as_path() == scroll_to_path.as_path())
}
