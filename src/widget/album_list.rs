use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use egui::{RichText, Ui};

use crate::{
    dependencies::{Dependency, SingletonFor},
    model::album::AlbumId,
    photo_manager::PhotoManager,
    widget::tree_list::{ROW_HEIGHT, SelectionStyle, TreeList, TreeListRow},
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
    pub selected_photo: Option<PathBuf>,
}

impl Default for AlbumListState {
    fn default() -> Self {
        Self {
            selected_album: None,
            expanded_albums: HashSet::new(),
            selected_photo: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AlbumListResponse {
    pub selected: Option<AlbumId>,
    pub selected_photo: Option<PathBuf>,
    pub double_clicked_photo: Option<PathBuf>,
}

pub struct AlbumList<'a> {
    state: &'a mut AlbumListState,
}

impl<'a> AlbumList<'a> {
    pub fn new(state: &'a mut AlbumListState) -> Self {
        Self { state }
    }

    pub fn show(&mut self, ui: &mut Ui) -> AlbumListResponse {
        ui.style_mut().interaction.selectable_labels = false;

        let mut selected_album = None;
        let mut selected_photo = None;
        let mut double_clicked_photo = None;

        let (current_album_filter, mut albums): (Option<AlbumId>, Vec<(AlbumId, String, usize)>) =
            Dependency::<PhotoManager>::get().with_lock(|pm| {
                let current_album_filter = pm.get_current_filter().album.clone();
                let albums = pm
                    .albums_iter()
                    .map(|album| (album.id.clone(), album.name.clone(), album.photos.len()))
                    .collect();

                (current_album_filter, albums)
            });

        if self.state.selected_album != current_album_filter {
            self.state.selected_album = current_album_filter.clone();
            self.state.selected_photo = None;
        }

        albums.sort_by_cached_key(|(_, name, _)| name.to_lowercase());

        if albums.is_empty() {
            ui.label(RichText::new("No albums").weak());
        } else {
            let layout = AlbumListLayout::new(albums, &self.state.expanded_albums);
            let mut photo_windows = HashMap::new();
            let mut disclosure_clicked = None;
            let mut clicked = None;
            let mut double_clicked = None;

            TreeList::new(ui).id_salt("album_list_scroll").body(|body| {
                body.rows(ROW_HEIGHT, layout.row_count, |mut row| {
                    let Some(tree_row) =
                        self.row_for_index(&layout, row.index(), &mut photo_windows)
                    else {
                        return;
                    };

                    let response = row.add(tree_row);

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
                        self.state.selected_photo = None;
                        if current_album_filter.as_ref() != Some(&album_id) {
                            selected_album = Some(album_id);
                        }
                    }
                    AlbumListRowId::Photo { album_id, path } => {
                        self.state.selected_album = Some(album_id.clone());
                        if current_album_filter.as_ref() != Some(&album_id) {
                            selected_album = Some(album_id);
                        }
                        self.state.selected_photo = Some(path.clone());
                        selected_photo = Some(path);
                    }
                }
            }

            if let Some(AlbumListRowId::Photo { album_id, path }) = double_clicked {
                self.state.selected_album = Some(album_id.clone());
                if current_album_filter.as_ref() != Some(&album_id) {
                    selected_album = Some(album_id);
                }
                self.state.selected_photo = Some(path.clone());
                selected_photo = Some(path.clone());
                double_clicked_photo = Some(path);
            }
        }

        AlbumListResponse {
            selected: selected_album,
            selected_photo,
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

        Some(
            TreeListRow::photo(
                AlbumListRowId::Photo {
                    album_id: section.album_id.clone(),
                    path: photo_path.clone(),
                },
                1,
                title,
                photo_path.clone(),
            )
            .selected(self.state.selected_photo.as_ref() == Some(&photo_path)),
        )
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
                Dependency::<PhotoManager>::get().with_lock_mut(|pm| {
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
