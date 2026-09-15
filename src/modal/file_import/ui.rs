use std::path::PathBuf;

use egui::{RichText, ScrollArea};

use crate::theme::color;

use super::{FileImportModal, NativeFileDialogTarget};

impl FileImportModal {
    pub(super) fn panel_frame() -> egui::Frame {
        egui::Frame::new()
            .fill(color::SURFACE_DARK)
            .stroke(egui::Stroke::new(1.0, color::SURFACE_MUTED))
            .corner_radius(8)
            .inner_margin(12)
    }

    fn directory_selectors_ui(&mut self, ui: &mut egui::Ui) {
        let selectors = [
            (
                NativeFileDialogTarget::Source,
                "Source folder",
                "Photos and metadata are read from here.",
                "Choose source folder…",
                &self.source_path,
            ),
            (
                NativeFileDialogTarget::Destination,
                "Destination folder",
                "The workflow builds the imported structure here.",
                "Choose destination folder…",
                &self.destination_path,
            ),
        ];
        let mut selected_target = None;
        let enabled = self.native_file_dialog.is_none();
        if ui.available_width() >= 700.0 {
            ui.columns(2, |columns| {
                for (ui, (target, title, description, placeholder, path)) in
                    columns.iter_mut().zip(selectors)
                {
                    if Self::directory_selector_ui(
                        ui,
                        title,
                        description,
                        placeholder,
                        path,
                        enabled,
                    ) {
                        selected_target = Some(target);
                    }
                }
            });
        } else {
            for (target, title, description, placeholder, path) in selectors {
                if Self::directory_selector_ui(ui, title, description, placeholder, path, enabled) {
                    selected_target = Some(target);
                }
            }
        }
        if let Some(target) = selected_target {
            self.open_native_file_dialog(target, ui.ctx());
        }
    }

    fn directory_selector_ui(
        ui: &mut egui::Ui,
        title: &str,
        description: &str,
        placeholder: &str,
        path: &Option<PathBuf>,
        enabled: bool,
    ) -> bool {
        let mut clicked = false;
        Self::panel_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().color(color::WHITE));
            ui.add_space(2.0);

            let display_path = path
                .as_deref()
                .map_or_else(|| placeholder.to_owned(), |path| path.display().to_string());
            if !enabled {
                ui.disable();
            }
            let response = ui.add_sized(
                egui::vec2(ui.available_width(), 32.0),
                egui::Button::new(display_path)
                    .truncate()
                    .fill(color::SURFACE_XX_DARK)
                    .stroke(egui::Stroke::new(1.0, color::SURFACE_MUTED)),
            );
            let response = if let Some(selected_path) = path.as_deref() {
                response.on_hover_text(format!(
                    "{}\n\nClick to choose another folder",
                    selected_path.display()
                ))
            } else {
                response.on_hover_text("Click to choose a folder")
            };
            clicked = response.clicked();
            ui.small(RichText::new(description).color(color::CONTROL_TEXT));
        });
        clicked
    }

    pub(super) fn import_body_ui(&mut self, ui: &mut egui::Ui) {
        let viewport = ui.ctx().content_rect().size();
        ui.set_width((viewport.x - 64.0).clamp(240.0, 1500.0));
        let body_height = (viewport.y - 210.0).clamp(180.0, 820.0);
        ScrollArea::vertical()
            .id_salt("import_dialog_body")
            .max_height(body_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let body_top = ui.cursor().top();
                ui.spacing_mut().item_spacing = egui::vec2(12.0, 8.0);
                self.directory_selectors_ui(ui);

                ui.add_space(4.0);
                Self::panel_frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    self.saved_workflows_ui(ui);
                });
                self.editor_panels_ui(ui, body_height - (ui.cursor().top() - body_top));
            });
        self.ensure_preview(ui.ctx());
        ui.separator();
        ui.checkbox(
            &mut self.add_to_collection,
            "Add imported images to collection",
        );
    }

    fn editor_panels_ui(&mut self, ui: &mut egui::Ui, available_height: f32) {
        let graph_visible = self.saved_workflows.graph_visible();
        let total_width = ui.available_width();
        let stacked = graph_visible && total_width < 900.0;
        let editor_height = available_height.max(if stacked { 560.0 } else { 320.0 });
        let (editor_rect, _) =
            ui.allocate_exact_size(egui::vec2(total_width, editor_height), egui::Sense::hover());
        let (graph_rect, preview_rect) = if !graph_visible {
            (None, editor_rect)
        } else if stacked {
            let graph_height = editor_height * 0.62;
            (
                Some(egui::Rect::from_min_size(
                    editor_rect.min,
                    egui::vec2(total_width, graph_height),
                )),
                egui::Rect::from_min_max(
                    editor_rect.min + egui::vec2(0.0, graph_height + 12.0),
                    editor_rect.max,
                ),
            )
        } else {
            let preview_width = (total_width * 0.26).clamp(280.0, 360.0);
            let graph_width = total_width - preview_width - 12.0;
            (
                Some(egui::Rect::from_min_size(
                    editor_rect.min,
                    egui::vec2(graph_width, editor_height),
                )),
                egui::Rect::from_min_max(
                    editor_rect.min + egui::vec2(graph_width + 12.0, 0.0),
                    editor_rect.max,
                ),
            )
        };
        if let Some(graph_rect) = graph_rect {
            ui.scope_builder(
                egui::UiBuilder::new()
                    .id_salt("workflow_panel")
                    .max_rect(graph_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| {
                    ui.set_clip_rect(ui.clip_rect().intersect(graph_rect));
                    Self::panel_frame().show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        ui.horizontal(|ui| {
                            ui.strong("Workflow");
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("Fit workflow").clicked() {
                                        self.graph.fit_requested = true;
                                    }
                                },
                            );
                        });
                        ui.weak("Follow the steps from left to right. Use + to insert a step.");
                        ui.separator();
                        ui.push_id(self.saved_workflows.revision, |ui| {
                            Self::workflow_graph_ui(ui, &mut self.workflow, &mut self.graph);
                        });
                    });
                },
            );
        }
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
}
