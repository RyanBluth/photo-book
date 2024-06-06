use eframe::egui;
use egui::{Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};

use egui_extras::Column;

use crate::template::{self, Template};

use super::spacer::Spacer;

pub enum TemplatesResponse {
    None,
    SelectTemplate(Template),
}

#[derive(Debug, PartialEq)]
pub struct TemplatesState {
    pub templates: Vec<Template>,
}

impl TemplatesState {
    pub fn new() -> TemplatesState {
        TemplatesState {
            templates: template::BUILT_IN.clone(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Templates<'a> {
    pub state: &'a mut TemplatesState,
}

impl<'a> Templates<'a> {
    pub fn new(state: &'a mut TemplatesState) -> Templates<'a> {
        Templates { state }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> TemplatesResponse {
        ui.spacing_mut().item_spacing = Vec2::splat(10.0);

        let window_width = ui.available_width();
        let window_height = ui.available_height();
        let column_width = 256.0;
        let row_height = 256.0;
        let num_columns: usize = (window_width / column_width).floor() as usize;

        //let padding_size = num_columns as f32 * 10.0;
        let spacer_width = (window_width
            - ((column_width + ui.spacing().item_spacing.x) * num_columns as f32)
            - 10.0
            - ui.spacing().item_spacing.x)
            .max(0.0);

        let num_rows = self.state.templates.len().div_ceil(num_columns);

        let mut clicked_template = None;

        egui_extras::TableBuilder::new(ui)
            .min_scrolled_height(window_height)
            .columns(Column::exact(column_width), num_columns)
            .column(Column::exact(spacer_width))
            .body(|body| {
                body.rows(row_height, num_rows, |mut row| {
                    let offest = row.index() * num_columns;
                    for i in 0..num_columns {
                        if offest + i >= self.state.templates.len() {
                            break;
                        }

                        let template = self.state.templates.get(offest + i).unwrap();

                        if row
                            .col(|ui| {
                                TemplatePreview::show(ui, template);
                            })
                            .1
                            .interact(Sense::click())
                            .double_clicked()
                        {
                            clicked_template = Some(template.clone());
                        }
                    }

                    row.col(|ui| {
                        ui.add(Spacer::new(spacer_width, row_height));
                    });
                })
            });

        if let Some(template) = clicked_template {
            TemplatesResponse::SelectTemplate(template)
        } else {
            TemplatesResponse::None
        }
    }
}

pub struct TemplatePreview {}

impl TemplatePreview {
    pub fn show(ui: &mut egui::Ui, template: &Template) {
        ui.vertical(|ui| {
            ui.label(template.name.clone());

            let available_rect = ui.available_rect_before_wrap();

            let page_rect = if template.page.aspect_ratio() > 1.0 {
                let width = available_rect.width();
                let height = width / template.page.aspect_ratio();
                Rect::from_min_size(available_rect.min, Vec2::new(width, height))
            } else {
                let height = available_rect.height();
                let width = height * template.page.aspect_ratio();
                Rect::from_min_size(available_rect.min, Vec2::new(width, height))
            };

            ui.painter().rect_filled(page_rect, 0.0, Color32::WHITE);

            let scale = page_rect.width() / template.page.size_pixels().x;

            for region in &template.regions {
                let region_rect = Rect::from_min_size(
                    Pos2::new(
                        page_rect.left() + region.relative_position.x * page_rect.width(),
                        page_rect.top() + region.relative_position.y * page_rect.height(),
                    ),
                    region.relative_size * page_rect.size(),
                );

                match &region.kind {
                    template::TemplateRegionKind::Image => {
                        ui.painter()
                            .rect_filled(region_rect, 0.0, Color32::LIGHT_BLUE);
                    }
                    template::TemplateRegionKind::Text {
                        sample_text,
                        font_size,
                    } => {
                        ui.painter().rect_stroke(
                            region_rect,
                            0.0,
                            Stroke::new(2.0, Color32::DARK_GRAY),
                        );

                        ui.allocate_ui_at_rect(region_rect, |ui| {
                            ui.label(
                                RichText::new(sample_text.clone())
                                    .font(FontId::proportional(*font_size * scale)),
                            );
                        });
                    }
                }
            }
        });
    }
}
