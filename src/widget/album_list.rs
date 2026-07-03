use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use egui::{RichText, Ui};

use crate::{
    dep, dep_mut,
    model::album::AlbumId,
    photo_manager::PhotoManager,
    selection_manager::SelectionModifiers,
    widget::tree_list::{SelectionStyle, TreeList, TreeListRow, TreeListSelection, ROW_HEIGHT},
};

const PHOTO_WINDOW_SIZE: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum AlbumListRowId {
    Album(AlbumId),
    Photo { album_id: AlbumId, path: PathBuf },
}

#[derive(Debug, Clone)]
struct AlbumListSection {
    album_id: AlbumId,
    name: String,
    photo_count: usize,
    is_expanded: bool,
    start_row: usize,
    row_count: usize,
}

#[derive(Debug, Clone)]
struct AlbumListLayout {
    sections: Vec<AlbumListSection>,
    row_count: usize,
}

impl AlbumListLayout {
    fn new(albums: Vec<(AlbumId, String, usize)>, expanded_albums: &HashSet<AlbumId>) -> Self {
        let mut sections = Vec::with_capacity(albums.len());
        let mut start_row = 0;

        for (album_id, name, photo_count) in albums {
            let is_expanded = expanded_albums.contains(&album_id);
            let row_count = 1 + if is_expanded { photo_count } else { 0 };

            sections.push(AlbumListSection {
                album_id,
                name,
                photo_count,
                is_expanded,
                start_row,
                row_count,
            });

            start_row += row_count;
        }

        Self {
            sections,
            row_count: start_row,
        }
    }

    fn section_for_row(&self, row_index: usize) -> Option<&AlbumListSection> {
        let section_index = self
            .sections
            .partition_point(|section| section.start_row <= row_index)
            .checked_sub(1)?;
        let section = &self.sections[section_index];

        (row_index < section.start_row + section.row_count).then_some(section)
    }
}

#[derive(Debug, Clone)]
pub struct AlbumListState {
    pub selected_album: Option<AlbumId>,
    pub expanded_albums: HashSet<AlbumId>,
}

impl Default for AlbumListState {
    fn default() -> Self {
        Self {
            selected_album: None,
            expanded_albums: HashSet::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AlbumListResponse {
    pub selected: Option<AlbumId>,
    pub double_clicked_photo: Option<PathBuf>,
}

pub struct AlbumList<'a> {
    state: &'a mut AlbumListState,
}

impl<'a> AlbumList<'a> {
    pub fn new(state: &'a mut AlbumListState) -> Self {
        Self { state }
    }

    pub fn show(&mut self, ui: &mut Ui, scroll_to_path: Option<&PathBuf>) -> AlbumListResponse {
        ui.style_mut().interaction.selectable_labels = false;

        let mut selection = TreeListSelection::new(ui);
        let mut selected_album = None;
        let mut double_clicked_photo = None;

        let (current_album_filter, mut albums): (Option<AlbumId>, Vec<(AlbumId, String, usize)>) =
            dep!(PhotoManager, |pm| {
                let current_album_filter = pm.get_current_filter().album.clone();
                let albums = pm
                    .albums_iter()
                    .map(|album| (album.id.clone(), album.name.clone(), album.photos.len()))
                    .collect();

                (current_album_filter, albums)
            });

        if self.state.selected_album != current_album_filter {
            self.state.selected_album = current_album_filter.clone();
            selection.clear_this_frame(ui);
        }

        albums.sort_by_cached_key(|(_, name, _)| name.to_lowercase());

        if albums.is_empty() {
            ui.label(RichText::new("No albums").weak());
        } else {
            let layout = AlbumListLayout::new(albums, &self.state.expanded_albums);
            let ordered_photo_paths = self.visible_photo_paths(&layout);
            let row_to_scroll = self.scroll_row_for_path(&layout, scroll_to_path);
            let mut photo_windows = HashMap::new();
            let mut disclosure_clicked = None;
            let mut clicked = None;
            let mut double_clicked = None;

            TreeList::new(ui)
                .id_salt("album_list_scroll")
                .scroll_to_row_top(row_to_scroll)
                .body(|body| {
                    body.rows(ROW_HEIGHT, layout.row_count, |mut row| {
                        let Some(tree_row) =
                            self.row_for_index(&layout, row.index(), &mut photo_windows)
                        else {
                            return;
                        };

                        let response =
                            row.add_selectable(tree_row, &mut selection, &ordered_photo_paths);

                        if response.disclosure_clicked() {
                            disclosure_clicked = Some(response.id().clone());
                        } else if response.clicked() {
                            clicked = Some(response.id().clone());
                        }

                        if response.double_clicked() {
                            double_clicked = Some(response.id().clone());
                        }
                    });
                });

            if let Some(AlbumListRowId::Album(album_id)) = disclosure_clicked {
                if self.state.expanded_albums.contains(&album_id) {
                    self.state.expanded_albums.remove(&album_id);
                } else {
                    self.state.expanded_albums.insert(album_id);
                }
            }

            if let Some(clicked) = clicked {
                match clicked {
                    AlbumListRowId::Album(album_id) => {
                        self.state.selected_album = Some(album_id.clone());
                        if current_album_filter.as_ref() != Some(&album_id) {
                            selected_album = Some(album_id);
                        }
                    }
                    AlbumListRowId::Photo { .. } => {}
                }
            }

            if let Some(AlbumListRowId::Photo { path, .. }) = double_clicked {
                selection.select_path(
                    ui,
                    &ordered_photo_paths,
                    &path,
                    SelectionModifiers::default(),
                );
                double_clicked_photo = Some(path);
            }
        }

        AlbumListResponse {
            selected: selected_album,
            double_clicked_photo,
        }
    }

    fn row_for_index(
        &self,
        layout: &AlbumListLayout,
        row_index: usize,
        photo_windows: &mut HashMap<(AlbumId, usize), Vec<PathBuf>>,
    ) -> Option<TreeListRow<AlbumListRowId>> {
        let section = layout.section_for_row(row_index)?;

        if row_index == section.start_row {
            return Some(
                TreeListRow::header(
                    AlbumListRowId::Album(section.album_id.clone()),
                    0,
                    section.name.clone(),
                    section.photo_count > 0,
                    section.is_expanded,
                )
                .selected(self.state.selected_album.as_ref() == Some(&section.album_id))
                .selection_style(SelectionStyle::BoldText)
                .trailing(section.photo_count.to_string()),
            );
        }

        let photo_index = row_index - section.start_row - 1;
        let photo_path =
            self.album_photo_path(section.album_id.clone(), photo_index, photo_windows)?;
        let title = photo_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        Some(TreeListRow::photo(
            AlbumListRowId::Photo {
                album_id: section.album_id.clone(),
                path: photo_path.clone(),
            },
            1,
            title,
            photo_path.clone(),
        ))
    }

    fn scroll_row_for_path(
        &self,
        layout: &AlbumListLayout,
        scroll_to_path: Option<&PathBuf>,
    ) -> Option<usize> {
        let scroll_to_path = scroll_to_path?;

        dep_mut!(PhotoManager, |pm| {
            let mut fallback_album_row = None;

            for section in &layout.sections {
                let Some(photo_index) = pm
                    .album_photos_iter(&section.album_id)
                    .position(|photo_path| photo_path == scroll_to_path)
                else {
                    continue;
                };

                if section.is_expanded {
                    return Some(section.start_row + photo_index + 1);
                }

                fallback_album_row.get_or_insert(section.start_row);
            }

            fallback_album_row
        })
    }

    fn visible_photo_paths(&self, layout: &AlbumListLayout) -> Vec<PathBuf> {
        dep_mut!(PhotoManager, |pm| {
            let mut paths = Vec::new();

            // TODO: Avoid rebuilding all visible paths every frame; range selection only needs this on selection input.
            for section in layout.sections.iter().filter(|section| section.is_expanded) {
                paths.extend(pm.album_photos_iter(&section.album_id).cloned());
            }

            paths
        })
    }

    fn album_photo_path(
        &self,
        album_id: AlbumId,
        photo_index: usize,
        photo_windows: &mut HashMap<(AlbumId, usize), Vec<PathBuf>>,
    ) -> Option<PathBuf> {
        let window_start = (photo_index / PHOTO_WINDOW_SIZE) * PHOTO_WINDOW_SIZE;
        let window = photo_windows
            .entry((album_id.clone(), window_start))
            .or_insert_with(|| {
                dep_mut!(PhotoManager, |pm| {
                    pm.album_photos_iter(&album_id)
                        .skip(window_start)
                        .take(PHOTO_WINDOW_SIZE)
                        .cloned()
                        .collect()
                })
            });

        window.get(photo_index - window_start).cloned()
    }
}
