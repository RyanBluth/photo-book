use crate::{id::LayerId, photo::Photo, widget::canvas_info::layers::Layer};
use eframe::epaint::{Pos2, Rect, Vec2};

pub enum CanvasResponse {
    Exit,
    EnterCropMode {
        target_layer: LayerId,
        photo: CanvasPhoto,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Select,
    Text,
    Rectangle,
    Ellipse,
    Line,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleTool {
    Select,
    Text,
    Rectangle,
    Ellipse,
    Line,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActiveTool {
    Select { start_pos: Pos2 },
    Text { start_pos: Pos2 },
    Rectangle { start_pos: Pos2 },
    Ellipse { start_pos: Pos2 },
    Line { start_pos: Pos2 },
}

impl From<ToolKind> for IdleTool {
    fn from(value: ToolKind) -> Self {
        match value {
            ToolKind::Select => IdleTool::Select,
            ToolKind::Text => IdleTool::Text,
            ToolKind::Rectangle => IdleTool::Rectangle,
            ToolKind::Ellipse => IdleTool::Ellipse,
            ToolKind::Line => IdleTool::Line,
        }
    }
}

impl From<&IdleTool> for ToolKind {
    fn from(value: &IdleTool) -> Self {
        match value {
            IdleTool::Select => ToolKind::Select,
            IdleTool::Text => ToolKind::Text,
            IdleTool::Rectangle => ToolKind::Rectangle,
            IdleTool::Ellipse => ToolKind::Ellipse,
            IdleTool::Line => ToolKind::Line,
        }
    }
}

impl From<&ActiveTool> for ToolKind {
    fn from(value: &ActiveTool) -> Self {
        match value {
            ActiveTool::Select { .. } => ToolKind::Select,
            ActiveTool::Text { .. } => ToolKind::Text,
            ActiveTool::Rectangle { .. } => ToolKind::Rectangle,
            ActiveTool::Ellipse { .. } => ToolKind::Ellipse,
            ActiveTool::Line { .. } => ToolKind::Line,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolState {
    Idle(IdleTool),
    Active(ActiveTool),
}

impl ToolState {
    pub fn tool_kind(&self) -> ToolKind {
        match self {
            ToolState::Idle(tool) => tool.into(),
            ToolState::Active(tool) => tool.into(),
        }
    }

    pub fn is_idle(&self) -> bool {
        matches!(self, ToolState::Idle(_))
    }

    pub fn is_active(&self) -> bool {
        matches!(self, ToolState::Active(_))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CreatingLayer {
    /// Line being created - stored here for live preview
    Line(Layer, Pos2),
    /// Other layer being created - stored in layers map with this ID
    Other(LayerId, Pos2),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasPhoto {
    pub photo: Photo,
    // Normalized crop rect
    pub crop: Rect,
}

impl CanvasPhoto {
    pub fn new(photo: Photo) -> Self {
        Self {
            photo,
            crop: Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ActionBarAction {
    SwapCenters(LayerId, LayerId),
    SwapCentersAndBounds(LayerId, LayerId),
    SwapQuickLayoutPosition(LayerId, LayerId),
    Crop(LayerId),
}
