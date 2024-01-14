use eframe::egui::{Context, CursorIcon};

pub struct CursorManager {
    current_frame_cursor: Option<CursorIcon>,
}

impl CursorManager {

    pub fn new() -> Self {
        Self {
            current_frame_cursor: None,
        }
    }

    pub fn begin_frame(&mut self, ctx: &Context) {
        self.current_frame_cursor = None;
    }

    pub fn set_cursor(&mut self, cursor: CursorIcon) {
        self.current_frame_cursor = Some(cursor);
    }

    pub fn end_frame(&self, ctx: &Context) {
        if let Some(cursor) = self.current_frame_cursor {
            ctx.set_cursor_icon(cursor);
        }
    }
}
