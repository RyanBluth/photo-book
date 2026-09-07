use std::{collections::HashMap, path::PathBuf, sync::Arc};

use egui::Ui;

use crate::{
    theme::color,
    widget::tree_list::{ROW_HEIGHT, TreeList, TreeListRow},
};

use super::{
    FileImportModal,
    preview_tree::PreviewTreeItem,
    workflow::{CachedImportFile, FileImportWorkflow},
};

impl FileImportModal {
    pub(super) fn ensure_preview(&mut self, ctx: &egui::Context) {
        let (Some(source_path), Some(destination_path)) =
            (self.source_path.clone(), self.destination_path.clone())
        else {
            let mut state = self.preview_state.lock().unwrap();
            if state.active_key.is_some() {
                state.generation = state.generation.wrapping_add(1);
            }
            state.active_key = None;
            state.status = ImportPreviewStatus::Idle;
            return;
        };
        let key = ImportPreviewKey {
            source_path: source_path.clone(),
            destination_path: destination_path.clone(),
            workflow_signature: Self::workflow_config_signature(&self.workflow),
        };

        let Some((cached_files, generation)) = self.preview_state.lock().unwrap().start(&key)
        else {
            return;
        };

        let preview_state = self.preview_state.clone();
        let ctx = ctx.clone();
        let import = FileImportWorkflow {
            source_path: Some(source_path.clone()),
            destination_path: Some(destination_path),
            workflow: self.workflow.clone(),
        };
        tokio::task::spawn_blocking(move || {
            let files =
                cached_files.map_or_else(|| import.scan_source().map(Arc::new), |files| Ok(files));
            let result = files
                .as_ref()
                .map_err(ToString::to_string)
                .and_then(|files| {
                    import
                        .build_preview(files)
                        .map_err(|error| error.to_string())
                });

            let files = files.ok();
            preview_state
                .lock()
                .unwrap()
                .publish(generation, &key, source_path, files, result);
            ctx.request_repaint();
        });
    }

    pub(super) fn preview_ui(&mut self, ui: &mut Ui) {
        let status = self.preview_state.lock().unwrap().status.clone();
        egui::Frame::new()
            .fill(color::SURFACE_DARK)
            .stroke(egui::Stroke::new(1.0, color::SURFACE_MUTED))
            .corner_radius(8)
            .inner_margin(10)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(egui::RichText::new("Import preview").color(color::WHITE));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Refresh").clicked() {
                            let mut state = self.preview_state.lock().unwrap();
                            state.generation = state.generation.wrapping_add(1);
                            state.active_key = None;
                            state.cache_source = None;
                            state.cached_files = None;
                            state.status = ImportPreviewStatus::Idle;
                        }
                    });
                });
                ui.separator();
                match status {
                    ImportPreviewStatus::Idle => {
                        ui.weak("Choose source and destination folders to preview the import.");
                    }
                    ImportPreviewStatus::Loading => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Reading metadata and evaluating workflow…");
                        });
                    }
                    ImportPreviewStatus::Error(error) => {
                        ui.colored_label(color::ERROR, error);
                    }
                    ImportPreviewStatus::Ready(preview) => {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(format!("{} files", preview.imported_count));
                            if preview.dropped_count > 0 {
                                ui.weak(format!("{} filtered out", preview.dropped_count));
                            }
                        });
                        if !preview.conflicts.is_empty() {
                            ui.colored_label(
                                color::ERROR,
                                format!(
                                    "{} destination conflicts block import",
                                    preview.conflicts.len()
                                ),
                            );
                            for path in preview.conflicts.iter().take(3) {
                                ui.small(format!("• {}", path.display()));
                            }
                            if preview.conflicts.len() > 3 {
                                ui.weak(format!("…and {} more", preview.conflicts.len() - 3));
                            }
                        }
                        ui.add_space(4.0);
                        if preview.imported_count == 0 {
                            ui.weak("No files match the current workflow.");
                        } else {
                            self.preview_tree_ui(ui, &preview);
                        }
                    }
                }
            });
    }

    fn preview_tree_ui(&mut self, ui: &mut Ui, preview: &ImportPreview) {
        let visible_items = preview
            .items
            .iter()
            .filter(|item| {
                item.depth == 0
                    || item
                        .path
                        .parent()
                        .is_some_and(|parent| self.preview_expanded.contains(parent))
            })
            .collect::<Vec<_>>();
        let mut toggle_path = None;

        TreeList::new(ui)
            .id_salt("file_import_preview_tree")
            .min_width(280.0)
            .body(|body| {
                body.rows(ROW_HEIGHT, visible_items.len(), |mut row| {
                    let item = visible_items[row.index()];
                    let path = item.path.clone();
                    let title = if item.depth == 0 {
                        path.to_string_lossy().to_string()
                    } else {
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    };
                    let tree_row = if item.is_directory {
                        TreeListRow::header(
                            path.clone(),
                            item.depth,
                            title,
                            item.has_children,
                            self.preview_expanded.contains(&path),
                        )
                    } else {
                        TreeListRow::photo(
                            path.clone(),
                            item.depth,
                            title,
                            preview
                                .source_by_output
                                .get(&path)
                                .cloned()
                                .unwrap_or_else(|| path.clone()),
                        )
                    }
                    .sense(egui::Sense::click());
                    let response = row.add(tree_row);
                    if response.disclosure_clicked()
                        || item.is_directory && response.double_clicked()
                    {
                        toggle_path = Some(path);
                    }
                });
            });

        if let Some(path) = toggle_path {
            if self.preview_expanded.remove(&path) {
                self.preview_expanded
                    .retain(|expanded| !expanded.starts_with(&path));
            } else {
                self.preview_expanded.insert(path);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ImportPreviewKey {
    pub(super) source_path: PathBuf,
    pub(super) destination_path: PathBuf,
    pub(super) workflow_signature: u64,
}

#[derive(Clone)]
pub(super) struct ImportPreview {
    pub(super) items: Arc<Vec<PreviewTreeItem>>,
    pub(super) source_by_output: Arc<HashMap<PathBuf, PathBuf>>,
    pub(super) conflicts: Arc<Vec<PathBuf>>,
    pub(super) imported_count: usize,
    pub(super) dropped_count: usize,
}

#[derive(Clone, Default)]
pub(super) enum ImportPreviewStatus {
    #[default]
    Idle,
    Loading,
    Ready(ImportPreview),
    Error(String),
}

#[derive(Default)]
pub(super) struct ImportPreviewState {
    pub(super) worker_running: bool,
    pub(super) generation: u64,
    pub(super) active_key: Option<ImportPreviewKey>,
    pub(super) cache_source: Option<PathBuf>,
    pub(super) cached_files: Option<Arc<Vec<CachedImportFile>>>,
    pub(super) status: ImportPreviewStatus,
}

impl ImportPreviewState {
    /// Coalesce edits while a worker runs; its completion wakes the UI to start the latest request.
    pub(super) fn start(
        &mut self,
        key: &ImportPreviewKey,
    ) -> Option<(Option<Arc<Vec<CachedImportFile>>>, u64)> {
        if self.active_key.as_ref() != Some(key) {
            self.generation = self.generation.wrapping_add(1);
            self.active_key = Some(key.clone());
            self.status = ImportPreviewStatus::Loading;
        } else if !matches!(self.status, ImportPreviewStatus::Loading) {
            return None;
        }
        if self.worker_running {
            return None;
        }
        self.worker_running = true;
        let cached = (self.cache_source.as_ref() == Some(&key.source_path))
            .then(|| self.cached_files.clone())
            .flatten();
        Some((cached, self.generation))
    }

    pub(super) fn publish(
        &mut self,
        generation: u64,
        key: &ImportPreviewKey,
        source_path: PathBuf,
        files: Option<Arc<Vec<CachedImportFile>>>,
        result: Result<ImportPreview, String>,
    ) {
        self.worker_running = false;
        if self.generation != generation || self.active_key.as_ref() != Some(key) {
            return;
        }
        if let Some(files) = files {
            self.cache_source = Some(source_path);
            self.cached_files = Some(files);
        }
        self.status = match result {
            Ok(preview) => ImportPreviewStatus::Ready(preview),
            Err(error) => ImportPreviewStatus::Error(error),
        };
    }
}
