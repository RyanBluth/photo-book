use eframe::egui::{self, Ui, ImageButton, Image, Vec2, Sense, Response};
use crate::assets::Asset;
use crate::theme::color;
use crate::widget::canvas::Tool;

pub struct Toolbar<'a> {
    current_tool: &'a mut Tool,
}

impl<'a> Toolbar<'a> {
    pub fn new(current_tool: &'a mut Tool) -> Self {
        Self { current_tool }
    }

    pub fn show(&mut self, ui: &mut Ui) -> ToolbarResponse {
        let mut response = ToolbarResponse::None;
        let icon_size = Vec2::splat(24.0);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            // Select tool
            if self.tool_button(
                ui,
                Asset::icon_select(),
                Tool::Select,
                "Select (V)",
                icon_size,
            ) {
                *self.current_tool = Tool::Select;
                response = ToolbarResponse::ToolChanged(Tool::Select);
            }

            // Text tool
            if self.tool_button(
                ui,
                Asset::icon_text(),
                Tool::Text,
                "Text (T)",
                icon_size,
            ) {
                *self.current_tool = Tool::Text;
                response = ToolbarResponse::ToolChanged(Tool::Text);
            }

            // Rectangle tool
            if self.tool_button(
                ui,
                Asset::icon_rectangle(),
                Tool::Rectangle,
                "Rectangle (U)",
                icon_size,
            ) {
                *self.current_tool = Tool::Rectangle;
                response = ToolbarResponse::ToolChanged(Tool::Rectangle);
            }

            // Ellipse tool
            if self.tool_button(
                ui,
                Asset::icon_ellipse(),
                Tool::Ellipse,
                "Ellipse (O)",
                icon_size,
            ) {
                *self.current_tool = Tool::Ellipse;
                response = ToolbarResponse::ToolChanged(Tool::Ellipse);
            }

            // Line tool
            if self.tool_button(
                ui,
                Asset::icon_line(),
                Tool::Line,
                "Line (L)",
                icon_size,
            ) {
                *self.current_tool = Tool::Line;
                response = ToolbarResponse::ToolChanged(Tool::Line);
            }
        });

        response
    }

    fn tool_button(
        &self,
        ui: &mut Ui,
        icon: egui::ImageSource,
        tool: Tool,
        tooltip: &str,
        size: Vec2,
    ) -> bool {
        let is_active = *self.current_tool == tool;

        // Determine tint color based on active state
        let tint_color = if is_active {
            color::FOCUSED // Active tool - highlighted color
        } else {
            color::PLACEHOLDER // Inactive tool - muted color
        };

        let image = Image::new(icon)
            .tint(tint_color)
            .fit_to_exact_size(size);

        let button = ImageButton::new(image);

        let response = ui.add(button).on_hover_text(tooltip);

        response.clicked()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarResponse {
    None,
    ToolChanged(Tool),
}
