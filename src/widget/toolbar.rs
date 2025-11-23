use crate::cursor_manager::{self, CursorManager};
use crate::dependencies::{Dependency, SingletonFor};
use crate::theme::color;
use crate::widget::canvas::Tool;
use crate::{assets::Asset, utils::RectExt};
use eframe::egui::{self, Image, ImageButton, Response, Sense, Ui, Vec2};
use egui::{Color32, CursorIcon, Rect};

pub struct Toolbar<'a> {
    current_tool: &'a mut Tool,
}

impl<'a> Toolbar<'a> {
    pub fn new(current_tool: &'a mut Tool) -> Self {
        Self { current_tool }
    }

    pub fn show(&mut self, ui: &mut Ui) -> ToolbarResponse {
        let mut response = ToolbarResponse::None;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);

            // Select tool
            if self.tool_button(ui, Asset::icon_select(), Tool::Select, "Select (V)") {
                *self.current_tool = Tool::Select;
                response = ToolbarResponse::ToolChanged(Tool::Select);
            }

            // Text tool
            if self.tool_button(ui, Asset::icon_text(), Tool::Text, "Text (T)") {
                *self.current_tool = Tool::Text;
                response = ToolbarResponse::ToolChanged(Tool::Text);
            }

            // Rectangle tool
            if self.tool_button(
                ui,
                Asset::icon_rectangle(),
                Tool::Rectangle,
                "Rectangle (U)",
            ) {
                *self.current_tool = Tool::Rectangle;
                response = ToolbarResponse::ToolChanged(Tool::Rectangle);
            }

            // Ellipse tool
            if self.tool_button(ui, Asset::icon_ellipse(), Tool::Ellipse, "Ellipse (O)") {
                *self.current_tool = Tool::Ellipse;
                response = ToolbarResponse::ToolChanged(Tool::Ellipse);
            }

            // Line tool
            if self.tool_button(ui, Asset::icon_line(), Tool::Line, "Line (L)") {
                *self.current_tool = Tool::Line;
                response = ToolbarResponse::ToolChanged(Tool::Line);
            }
        });

        response
    }

    fn tool_button(&self, ui: &mut Ui, icon: egui::ImageSource, tool: Tool, tooltip: &str) -> bool {
        let is_active = *self.current_tool == tool;

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
    ToolChanged(Tool),
}
