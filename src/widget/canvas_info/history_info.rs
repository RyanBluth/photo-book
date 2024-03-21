use eframe::egui::{self};
use egui::{ComboBox, RichText, Vec2};

use strum::IntoEnumIterator;

use crate::{
    history,
    scene::canvas_scene::CanvasHistoryManager,
    utils::EditableValueTextEdit,
    widget::page_canvas::{Page, Unit},
};

#[derive(Debug, PartialEq)]
pub struct HistoryInfoState<'a> {
    history_manager: &'a mut CanvasHistoryManager,
}

impl HistoryInfoState<'_> {
    pub fn new(history_manager: &mut CanvasHistoryManager) -> HistoryInfoState {
        HistoryInfoState { history_manager }
    }
}

#[derive(Debug, PartialEq)]
pub struct HistoryInfo<'a> {
    pub state: &'a mut HistoryInfoState<'a>,
}

impl<'a> HistoryInfo<'a> {
    pub fn new(state: &'a mut HistoryInfoState<'a>) -> HistoryInfo<'a> {
        HistoryInfo { state }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.style_mut().spacing.text_edit_width = 80.0;

            ui.label(RichText::new("History").heading());

            if self.state.history_manager.is_at_end() {
                ui.label(
                    RichText::new("Unsaved changes")
                        .monospace()
                        .color(egui::Color32::GREEN),
                );
                ui.separator();
            }

            for history in self
                .state
                .history_manager
                .stack
                .history
                .iter()
                .enumerate()
                .rev()
            {
                ui.horizontal(|ui| {
                    ui.label(format!("{}", history.0));
                    ui.label(RichText::new(history.1 .0.to_string()).monospace().color(
                        if history.0 == self.state.history_manager.stack.index {
                            egui::Color32::GREEN
                        } else {
                            egui::Color32::WHITE
                        },
                    ));
                });

                ui.separator();
            }
        });
    }
}
