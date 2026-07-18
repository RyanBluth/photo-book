use crate::assets::Asset;
use crate::widget::canvas::types::ToolKind;
use crate::widget::icon_button::IconButton;
use eframe::egui::{self, Ui, Vec2, Widget};

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
        icon: egui::ImageSource<'static>,
        tool: ToolKind,
        tooltip: &str,
    ) -> bool {
        IconButton::new(icon)
            .size(Vec2::splat(28.0))
            .active(self.current_tool == tool)
            .ui(ui)
            .on_hover_text(tooltip)
            .clicked()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarResponse {
    None,
    ToolChanged(ToolKind),
}
