use egui::{Button, CursorIcon, Frame, RichText, Sense, TextStyle, UiBuilder, Vec2};

use crate::{cursor_manager::CursorManager, dep_mut, theme::color};

pub const SECTION_HEADER_INNER_MARGIN: f32 = 8.0;

pub fn section_header_height(ui: &egui::Ui) -> f32 {
    ui.text_style_height(&TextStyle::Body)
        .max(ui.spacing().interact_size.y)
        + SECTION_HEADER_INNER_MARGIN * 2.0
}

#[derive(Debug, Clone)]
pub struct CollapsableSectionState {
    pub expanded: bool,
    pub title: String,
}

impl CollapsableSectionState {
    pub fn new(expanded: bool, title: String) -> Self {
        Self { expanded, title }
    }
}

pub struct CollapsableSection<'a> {
    pub state: &'a mut CollapsableSectionState,
}

pub enum CollapsableSectionResponse {
    None,
    Expanded,
    Collapsed,
}

impl<'a> CollapsableSection<'a> {
    pub fn new(state: &'a mut CollapsableSectionState) -> Self {
        Self { state }
    }

    #[allow(dead_code)]
    pub fn show<R>(
        &mut self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui) -> R + 'a,
    ) -> CollapsableSectionResponse {
        self.show_with_action(ui, None, add_contents)
    }

    pub fn show_with_action<R>(
        &mut self,
        ui: &mut egui::Ui,
        header_action: Option<(&str, &str, &mut dyn FnMut())>,
        add_contents: impl FnOnce(&mut egui::Ui) -> R + 'a,
    ) -> CollapsableSectionResponse {
        ui.vertical(|ui| {
            let header_height = section_header_height(ui);
            let (header_rect, _) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), header_height),
                Sense::hover(),
            );
            ui.painter()
                .rect_filled(header_rect, 0.0, color::SURFACE_DARK);

            let inner_rect = header_rect.shrink(SECTION_HEADER_INNER_MARGIN);
            let mut header_ui = ui.new_child(
                UiBuilder::new()
                    .max_rect(inner_rect)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            header_ui.style_mut().interaction.selectable_labels = false;

            if let Some((icon, tooltip, on_click)) = header_action {
                let action_size = Vec2::splat(header_ui.available_height());
                let action_response = header_ui
                    .add_sized(action_size, Button::new(icon).frame(false))
                    .on_hover_text(tooltip);

                if action_response.clicked() {
                    on_click();
                }
            }

            let title_width = header_ui.available_width();
            let (title_rect, response) = header_ui.allocate_exact_size(
                Vec2::new(title_width, header_ui.available_height()),
                Sense::click(),
            );
            let mut title_ui = header_ui.new_child(
                UiBuilder::new()
                    .max_rect(title_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            title_ui.label(RichText::new(&self.state.title).strong());

            if self.state.expanded {
                Frame::new().inner_margin(8.0).show(ui, |ui| {
                    (add_contents)(ui);
                });
            }

            if response.hovered() {
                dep_mut!(CursorManager, |cursor_manager| {
                    cursor_manager.set_cursor(CursorIcon::PointingHand);
                });
            }

            if response.clicked() {
                self.state.expanded = !self.state.expanded;
                if self.state.expanded {
                    return CollapsableSectionResponse::Expanded;
                } else {
                    return CollapsableSectionResponse::Collapsed;
                }
            }
            return CollapsableSectionResponse::None;
        })
        .inner
    }
}
