use egui::{RichText, Ui};
use egui_extras::{Column, TableBuilder};

use crate::{string_log::StringLog, theme::color};

const ROW_MIN_HEIGHT: f32 = 22.0;

pub struct LogViewer<'a> {
    log: &'a StringLog,
    id_salt: egui::Id,
}

impl<'a> LogViewer<'a> {
    pub fn new(log: &'a StringLog) -> Self {
        Self {
            log,
            id_salt: egui::Id::new("log_viewer"),
        }
    }

    pub fn id_salt(mut self, id_salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        self.id_salt = egui::Id::new(id_salt);
        self
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let mut lines = Vec::new();
        self.log._for_each(|line| lines.push(line.clone()));

        if lines.is_empty() {
            ui.label(RichText::new("No log entries").weak());
            return;
        }

        TableBuilder::new(ui)
            .id_salt(self.id_salt)
            .striped(true)
            .resizable(false)
            .sense(egui::Sense::hover())
            .column(Column::remainder().clip(true))
            .body(|mut body| {
                let width = body.ui_mut().clip_rect().width().max(50.0);
                let font_id = egui::TextStyle::Monospace.resolve(body.ui_mut().style());
                let row_heights = lines
                    .iter()
                    .map(|line| {
                        Self::layout_line(body.ui_mut(), line, width, font_id.clone())
                            .size()
                            .y
                            .max(ROW_MIN_HEIGHT)
                    })
                    .collect::<Vec<_>>();

                body.heterogeneous_rows(row_heights.iter().copied(), |mut row| {
                    let row_index = row.index();
                    let Some(line) = lines.get(row_index) else {
                        return;
                    };

                    row.col(|ui| {
                        Self::add_wrapped_line(ui, line, width, font_id.clone());
                    });
                });
            });
    }

    fn add_wrapped_line(ui: &mut Ui, line: &str, width: f32, font_id: egui::FontId) {
        let width = width
            .min(ui.clip_rect().right() - ui.next_widget_position().x)
            .max(50.0);

        let galley = Self::layout_line(ui, line, width, font_id);
        let (rect, _) = ui.allocate_exact_size(
            egui::Vec2::new(width, galley.size().y),
            egui::Sense::hover(),
        );

        if ui.is_rect_visible(rect) {
            ui.painter().galley(rect.min, galley, color_for_line(line));
        }
    }

    fn layout_line(
        ui: &mut Ui,
        line: &str,
        width: f32,
        font_id: egui::FontId,
    ) -> std::sync::Arc<egui::Galley> {
        let mut job =
            egui::text::LayoutJob::simple(line.to_owned(), font_id, color_for_line(line), width);
        let mut wrap = egui::text::TextWrapping::wrap_at_width(width);
        wrap.break_anywhere = true;
        job.wrap = wrap;

        ui.fonts_mut(|fonts| fonts.layout_job(job))
    }
}

fn color_for_line(line: &str) -> egui::Color32 {
    if line.starts_with("[ERROR]") {
        color::ERROR
    } else if line.starts_with("[WARN]") {
        color::WARNING
    } else {
        color::WHITE
    }
}
