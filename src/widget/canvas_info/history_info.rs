use eframe::egui::{self};
use egui::{RichText};

use egui_extras::{Column, TableBuilder};
use strum::IntoEnumIterator;

use crate::{
    scene::canvas_scene::{CanvasHistoryKind, CanvasHistoryManager}, utils::EguiExt,
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

            let history: Vec<&CanvasHistoryKind> = self
                .state
                .history_manager
                .stack
                .history
                .iter()
                .map(|(kind, _)| kind)
                .rev()
                .collect();

            let available_width = ui.available_width();

            TableBuilder::new(ui)
                .column(Column::exact(available_width))
                .striped(true)
                .body(|body| {
                    body.rows(
                        20.0,
                        self.state.history_manager.stack.history.len(),
                        |mut row| {
                            let index = row.index();
                            let history_kind = history[index];
                            row.col(|ui| {
                                if ui
                                    .clickable(|ui| {
                                        ui.set_width(available_width);
                                        ui.horizontal(|ui| {
                                            ui.label(format!("{}", index));
                                            ui.label(
                                                RichText::new(history_kind.to_string())
                                                    .monospace()
                                                    .color(
                                                        if (history.len() - 1) - index
                                                            == self
                                                                .state
                                                                .history_manager
                                                                .stack
                                                                .index
                                                        {
                                                            egui::Color32::GREEN
                                                        } else {
                                                            egui::Color32::WHITE
                                                        },
                                                    ),
                                            );
                                        })
                                    })
                                    .response
                                    .clicked()
                                {
                                    self.state.history_manager.stack.index =
                                        (history.len() - 1) - index;
                                }
                            });
                        },
                    );
                });
        });
    }
}