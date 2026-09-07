mod graph;
mod graph_model;
mod node_editor;
mod preview;
mod preview_tree;
mod workflow;

#[cfg(test)]
mod tests;

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::{
    dep_mut,
    modal::{
        Modal, ModalActionResponse, basic::BasicModal, manager::ModalManager,
        progress::ProgressModal,
    },
    theme::{color, style},
};

use self::{
    graph::WorkflowGraphState,
    preview::ImportPreviewState,
    workflow::{FileImportWorkflow, FileWorkflow},
};

#[allow(unused_imports)]
pub use workflow::FileImportWorkflowError;

pub struct FileImportModal {
    source_path: Option<PathBuf>,
    destination_path: Option<PathBuf>,
    workflow: FileWorkflow,
    graph: WorkflowGraphState,
    preview_state: Arc<Mutex<ImportPreviewState>>,
    preview_expanded: HashSet<PathBuf>,
}

impl FileImportModal {
    pub fn new() -> Self {
        Self {
            source_path: None,
            destination_path: None,
            workflow: FileWorkflow::default(),
            graph: WorkflowGraphState::default(),
            preview_state: Arc::new(Mutex::new(ImportPreviewState::default())),
            preview_expanded: HashSet::new(),
        }
    }

    fn directory_selector_ui(
        ui: &mut egui::Ui,
        title: &str,
        description: &str,
        placeholder: &str,
        path: &mut Option<PathBuf>,
    ) {
        egui::Frame::new()
            .fill(color::SURFACE_DARK)
            .stroke(egui::Stroke::new(1.0, color::SURFACE_MUTED))
            .corner_radius(8)
            .inner_margin(12)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(egui::RichText::new(title).strong().color(color::WHITE));
                ui.weak(description);
                ui.add_space(5.0);

                let display_path = path
                    .as_deref()
                    .map_or_else(|| placeholder.to_owned(), |path| path.display().to_string());
                let selected_path = path.as_deref().map(Path::to_path_buf);
                let response = ui.add_sized(
                    egui::vec2(ui.available_width(), 36.0),
                    egui::Button::new(display_path)
                        .truncate()
                        .fill(color::SURFACE_XX_DARK)
                        .stroke(egui::Stroke::new(1.0, color::SURFACE_MUTED)),
                );
                let response = if let Some(selected_path) = selected_path {
                    response.on_hover_text(format!(
                        "{}\n\nClick to choose another folder",
                        selected_path.display()
                    ))
                } else {
                    response.on_hover_text("Click to choose a folder")
                };
                if response.clicked()
                    && let Ok(Some(selected_path)) = native_dialog::DialogBuilder::file()
                        .open_single_dir()
                        .show()
                {
                    *path = Some(selected_path);
                }
            });
    }
}

impl Modal for FileImportModal {
    type Response = ModalActionResponse;

    fn title(&self) -> String {
        "Import".to_string()
    }

    fn body_ui(&mut self, ui: &mut egui::Ui) {
        let viewport = ui.ctx().content_rect().size();
        // Also constrain the modal's following action row when the window shrinks.
        ui.set_width(viewport.x * 0.9);
        // Size a child region so setting its height cannot rewind the modal title's cursor.
        ui.scope(|ui| {
            // Reserve room for the modal's title, margins, and action buttons.
            ui.set_height((viewport.y - 180.0).max(260.0));
            ui.columns(2, |columns| {
                Self::directory_selector_ui(
                    &mut columns[0],
                    "Source folder",
                    "Photos and metadata are read from here.",
                    "Choose source folder…",
                    &mut self.source_path,
                );
                Self::directory_selector_ui(
                    &mut columns[1],
                    "Destination folder",
                    "The workflow builds the imported structure here.",
                    "Choose destination folder…",
                    &mut self.destination_path,
                );
            });

            ui.add_space(16.0);
            ui.heading("Workflow");
            ui.label("Build the import from left to right. Each file follows the connected steps.");
            ui.add_space(8.0);

            {
                let total_width = ui.available_width();
                let preview_width = 340.0_f32.min(total_width * 0.34);
                let graph_width = total_width - preview_width - 10.0;
                let editor_height = ui.available_height().max(120.0);
                let (editor_rect, _) = ui.allocate_exact_size(
                    egui::vec2(total_width, editor_height),
                    egui::Sense::hover(),
                );
                let graph_rect = egui::Rect::from_min_size(
                    editor_rect.min,
                    egui::vec2(graph_width, editor_height),
                );
                let preview_rect = egui::Rect::from_min_size(
                    egui::pos2(graph_rect.max.x + 10.0, editor_rect.min.y),
                    egui::vec2(preview_width, editor_height),
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .id_salt("workflow_editor")
                        .max_rect(graph_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        ui.set_clip_rect(ui.clip_rect().intersect(graph_rect));
                        Self::workflow_graph_ui(ui, &mut self.workflow, &mut self.graph);
                    },
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .id_salt("import_preview")
                        .max_rect(preview_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        ui.set_clip_rect(ui.clip_rect().intersect(preview_rect));
                        self.preview_ui(ui);
                    },
                );
            }
            self.ensure_preview(ui.ctx());
        });
    }

    fn actions_ui(&mut self, ui: &mut egui::Ui) -> Option<Self::Response> {
        let can_import = self.source_path.is_some() && self.destination_path.is_some();
        if style::primary_button_enabled(ui, can_import, "Import").clicked() {
            let workflow = FileImportWorkflow {
                source_path: self.source_path.clone(),
                destination_path: self.destination_path.clone(),
                workflow: self.workflow.clone(),
            };
            let ctx = ui.ctx().clone();

            tokio::task::spawn_blocking(move || {
                let progress_modal_id = ModalManager::push(ProgressModal::new(
                    "Importing",
                    "Copying files…",
                    "Run in Background",
                    0.0,
                ));
                ctx.request_repaint();

                let result = workflow.run();
                dep_mut!(ModalManager, |modal_manager| {
                    modal_manager.dismiss(progress_modal_id);
                });

                match result {
                    Ok(()) => {
                        ModalManager::push(BasicModal::new(
                            "Import Complete",
                            "The files were imported successfully.",
                            "Close",
                        ));
                    }
                    Err(error) => {
                        ModalManager::push(BasicModal::new(
                            "Import Failed",
                            error.to_string(),
                            "Close",
                        ));
                    }
                }
                ctx.request_repaint();
            });

            return Some(ModalActionResponse::Confirm);
        }

        if ui.button("Cancel").clicked() {
            return Some(ModalActionResponse::Cancel);
        }
        None
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
