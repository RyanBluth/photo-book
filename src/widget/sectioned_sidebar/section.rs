use egui::{CursorIcon, Frame, InnerResponse, Response, RichText, Sense, UiBuilder, Vec2, Widget};

use crate::{
    cursor_manager::{self, CursorManager},
    dependencies::{Dependency, SingletonFor},
    theme::color::ACTION_BAR,
};

pub const SECTION_HEADER_INNER_MARGIN: f32 = 8.0;

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

    pub fn show<R>(
        &mut self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui) -> R + 'a,
    ) -> CollapsableSectionResponse {
        ui.vertical(|ui| {
            let title_response = Frame::new()
                .fill(ACTION_BAR)
                .inner_margin(SECTION_HEADER_INNER_MARGIN)
                .show(ui, |ui| {
                    ui.scope_builder(UiBuilder::new().sense(Sense::click()), |ui| {
                        ui.allocate_at_least(
                            Vec2 {
                                x: ui.available_size_before_wrap().x,
                                y: 0.0,
                            },
                            Sense::click(),
                        );

                        ui.horizontal(|ui| {
                            ui.style_mut().interaction.selectable_labels = false;
                            ui.label(RichText::new(&self.state.title).strong());
                        })
                    })
                });
            if self.state.expanded {
                Frame::new().inner_margin(8.0).show(ui, |ui| {
                    (add_contents)(ui);
                });
            }

            if title_response.response.hovered() {
                Dependency::<CursorManager>::get().with_lock_mut(|cursor_manager| {
                    cursor_manager.set_cursor(CursorIcon::PointingHand);
                });
            }

            if title_response.inner.response.clicked() {
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
