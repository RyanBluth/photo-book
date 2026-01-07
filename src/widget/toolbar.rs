use crate::assets::Asset;
use crate::cursor_manager::CursorManager;
use crate::dependencies::{Dependency, SingletonFor};
use crate::theme::color;
use crate::widget::canvas::types::ToolKind;
use eframe::egui::{self, Image, Sense, Ui, Vec2};
use egui::CursorIcon;

pub struct Toolbar {
    current_tool: ToolKind,
}

impl Toolbar {
    pub fn new(current_tool: ToolKind) -> Self {
        Self { current_tool }
    }

    pub fn show(&mut self, ui: &mut Ui) -> ToolbarResponse {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);

            // Select tool
            if self.tool_button(ui, Asset::icon_select(), ToolKind::Select, "Select (V)") {
                return ToolbarResponse::ToolChanged(ToolKind::Select);
            }

            // Text tool
            if self.tool_button(ui, Asset::icon_text(), ToolKind::Text, "Text (T)") {
                return ToolbarResponse::ToolChanged(ToolKind::Text);
            }

            // Rectangle tool
            if self.tool_button(
                ui,
                Asset::icon_rectangle(),
                ToolKind::Rectangle,
                "Rectangle (U)",
            ) {
                return ToolbarResponse::ToolChanged(ToolKind::Rectangle);
            }

            // Ellipse tool
            if self.tool_button(ui, Asset::icon_ellipse(), ToolKind::Ellipse, "Ellipse (O)") {
                return ToolbarResponse::ToolChanged(ToolKind::Ellipse);
            }

            // Line tool
            if self.tool_button(ui, Asset::icon_line(), ToolKind::Line, "Line (L)") {
                return ToolbarResponse::ToolChanged(ToolKind::Line);
            }

            ToolbarResponse::None
        })
        .inner
    }

    fn tool_button(
        &self,
        ui: &mut Ui,
        icon: egui::ImageSource,
        tool: ToolKind,
        _tooltip: &str,
    ) -> bool {
        let is_active = self.current_tool == tool;

        let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());

        let background_color = if is_active {
            color::SELECTED_TOOL_BACKGROUND
        } else if response.hovered() {
            color::HOVER_TOOL_BACKGROUND
        } else {
            ui.style().visuals.window_fill()
        };

        if response.hovered() {
            Dependency::<CursorManager>::get().with_lock_mut(|cursor_manager| {
                cursor_manager.set_cursor(CursorIcon::PointingHand)
            });
        }

        ui.painter().rect_filled(rect, 2.0, background_color);

        let tint_color = if is_active {
            ui.style().visuals.window_fill()
        } else {
            color::SELECTED_TOOL_BACKGROUND
        };

        let image = Image::new(icon).tint(tint_color).shrink_to_fit();

        ui.put(rect.shrink(2.0), image);

        response.clicked()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarResponse {
    None,
    ToolChanged(ToolKind),
}
