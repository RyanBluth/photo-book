use std::collections::HashSet;
use std::path::PathBuf;

use egui::{Image, ImageSource, Rect, Response, RichText, Sense, Ui, UiBuilder, Vec2};
use egui_extras::{Column, TableBuilder};

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    file_tree::{FileTreeNode, FlattenedTreeItem},
    photo_manager::PhotoManager,
    theme,
};

const INDENT_WIDTH: f32 = 20.0;
const BASE_WIDTH: f32 = 200.0;
const THUMBNAIL_SIZE: f32 = 16.0;

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
    pub response: Response,

    pub selected: Option<PathBuf>,

    pub double_clicked: Option<PathBuf>,
}

impl<'a> FileTree<'a> {
    pub fn new(state: &'a mut FileTreeState) -> Self {
        Self { state }
    }

    pub fn show(&mut self, ui: &mut Ui, scroll_to_path: Option<&PathBuf>) -> FileTreeResponse {
        ui.style_mut().interaction.selectable_labels = false;

        let mut selected_path_this_frame: Option<PathBuf> = None;
        let mut double_clicked_path_this_frame: Option<PathBuf> = None;

        let outer_response = ui.allocate_response(ui.available_size(), egui::Sense::click());
        let mut table_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(outer_response.rect)
                .layout(*ui.layout()),
        );

        let items = Dependency::<PhotoManager>::get()
            .with_lock_mut(|pm| pm.file_collection.flattened_file_trees().clone());
        let visible_items: Vec<&FlattenedTreeItem> = items
            .iter()
            .filter(|item| self.is_path_visible(item))
            .collect();

        let max_depth = visible_items.len();

        let row_height = 24.0;
        let heights: Vec<f32> = vec![row_height; visible_items.len()];

        let column_width = BASE_WIDTH + (max_depth as f32 * INDENT_WIDTH);

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

        let mut builder = TableBuilder::new(&mut table_ui)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Min))
            .column(Column::exact(column_width))
            .resizable(true);

        if let Some(row) = row_to_scroll {
            builder = builder.scroll_to_row(row, None);
        }

        builder.body(|body| {
            body.heterogeneous_rows(heights.into_iter(), |mut row| {
                let row_index = row.index();
                if row_index < visible_items.len() {
                    let item = visible_items[row_index];
                    row.col(|ui| {
                        let (selected, double_clicked) = self.draw_tree_item(ui, item);
                        if let Some(path) = selected {
                            selected_path_this_frame = Some(path);
                        }
                        if let Some(path) = double_clicked {
                            double_clicked_path_this_frame = Some(path);
                        }
                    });
                }
            });
        });

        FileTreeResponse {
            response: outer_response,
            selected: selected_path_this_frame,
            double_clicked: double_clicked_path_this_frame,
        }
    }

    fn draw_tree_item(
        &mut self,
        ui: &mut egui::Ui,
        item: &FlattenedTreeItem,
    ) -> (Option<PathBuf>, Option<PathBuf>) {
        let mut selected_path: Option<PathBuf> = None;
        let mut double_clicked_path: Option<PathBuf> = None;

        let item_path = item.node.path().clone();

        let display_text = if item.is_root {
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

        let bg_color = if is_selected {
            Some(ui.visuals().selection.bg_fill)
        } else {
            None
        };

        let (rect, response) = ui.allocate_at_least(
            egui::vec2(ui.available_width(), 24.0),
            egui::Sense::click_and_drag(),
        );

        if let Some(color) = bg_color {
            ui.painter().rect_filled(rect, 0.0, color);
        }

        let mut content_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        let indent_space = item.depth as f32 * INDENT_WIDTH;

        content_ui.horizontal(|ui| {
            ui.add_space(indent_space);

            match &item.node {
                FileTreeNode::Directory(path, children) if !children.is_empty() => {
                    let is_expanded = self.state.expanded_directories.contains(path);

                    let icon = if is_expanded { '▼' } else { '▶' };
                    let arrow_response = ui.selectable_label(false, icon.to_string());

                    if arrow_response.clicked() {
                        if is_expanded {
                            self.state.expanded_directories.remove(path);
                            self.collapse_all_subdirectories(path);
                        } else {
                            self.state.expanded_directories.insert(path.clone());
                        }
                    }
                    ui.label(RichText::new(display_text));
                }
                FileTreeNode::File(path) => {
                    let photo_manager: Singleton<PhotoManager> = Dependency::get();

                    let photo_clone = photo_manager.with_lock(|pm| pm.photos.get(path).cloned());

                    let texture_handle = if let Some(photo) = photo_clone {
                        photo_manager.with_lock_mut(|pm| {
                            match pm.thumbnail_texture_for(&photo, ui.ctx()) {
                                Ok(Some(texture)) => Some(texture),
                                _ => None,
                            }
                        })
                    } else {
                        None
                    };

                    if let Some(handle) = texture_handle {
                        ui.add(
                            Image::new(ImageSource::Texture(handle))
                                .max_size(Vec2::splat(THUMBNAIL_SIZE)),
                        );
                    } else {
                        let next_pos = ui.next_widget_position();
                        let rect = Rect::from_min_max(
                            next_pos,
                            next_pos + Vec2::new(THUMBNAIL_SIZE, THUMBNAIL_SIZE),
                        );
                        ui.allocate_rect(rect, Sense::hover());
                        ui.painter()
                            .rect_filled(rect, 0.0, theme::color::PLACEHOLDER);
                    }
                    ui.add_space(4.0);
                    ui.label(RichText::new(display_text));
                }

                _ => {
                    // Add space for alignment where files/empty dirs don't have arrows
                    ui.add_space(14.0); // Approximate width of the arrow button
                    ui.label(RichText::new(display_text));
                }
            }
        });

        if response.clicked() {
            let new_selected = Some(item_path.clone());
            if self.state.selected_node != new_selected {
                self.state.selected_node = new_selected.clone();
                selected_path = new_selected;
            }
        }

        if response.double_clicked() {
            double_clicked_path = Some(item_path.clone());
        }

        (selected_path, double_clicked_path)
    }

    fn collapse_all_subdirectories(&mut self, path: &PathBuf) {
        let mut to_remove = Vec::new();

        for item in Dependency::<PhotoManager>::get()
            .with_lock_mut(|pm| pm.file_collection.flattened_file_trees().clone())
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
