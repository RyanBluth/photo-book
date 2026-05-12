use egui::{Button, Ui};

use crate::{
    dependencies::{Dependency, SingletonFor},
    photo_manager::PhotoManager,
};

#[derive(Debug, Clone)]
pub struct AlbumListState {
    pub selected_album: Option<String>,
}

impl Default for AlbumListState {
    fn default() -> Self {
        Self {
            selected_album: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AlbumListResponse {
    pub selected: Option<String>,
}

pub struct AlbumList<'a> {
    state: &'a mut AlbumListState,
}

impl<'a> AlbumList<'a> {
    pub fn new(state: &'a mut AlbumListState) -> Self {
        Self { state }
    }

    pub fn show(&mut self, ui: &mut Ui) -> AlbumListResponse {
        let mut selected_album: Option<String> = None;

        let albums: Vec<(String, usize)> = Dependency::<PhotoManager>::get().with_lock(|pm| {
            pm.photo_database
                .album_names_iter()
                .cloned()
                .map(|name| {
                    let count = pm.photo_database.album_photos_iter(&name).count();
                    (name, count)
                })
                .collect()
        });

        if albums.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("No albums");
            });
            return AlbumListResponse { selected: None };
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (name, count) in albums {
                let label = format!("{name} ({count})");
                let is_selected = self
                    .state
                    .selected_album
                    .as_ref()
                    .map(|selected| selected == &name)
                    .unwrap_or(false);

                let response = ui.add_sized(
                    [ui.available_width(), 24.0],
                    Button::new(label).selected(is_selected),
                );

                if response.clicked() && !is_selected {
                    self.state.selected_album = Some(name.clone());
                    selected_album = Some(name);
                }
            }
        });

        AlbumListResponse {
            selected: selected_album,
        }
    }
}
