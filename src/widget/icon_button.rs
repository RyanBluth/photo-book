use egui::{Image, ImageSource, Rect, Sense, Widget};

use crate::{
    cursor_manager::CursorManager,
    dep_mut,
    egui::{Color32, Vec2},
    theme,
};

pub struct IconButtonActiveState {
    icon: Option<ImageSource<'static>>,
    tint: Color32,
}

impl IconButtonActiveState {
    pub fn new(tint: Color32) -> Self {
        Self { icon: None, tint }
    }

    pub fn icon(mut self, icon: impl Into<ImageSource<'static>>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

pub struct IconButton {
    image_source: ImageSource<'static>,
    tint: Color32,
    active_state: IconButtonActiveState,
    active: bool,
    size: Vec2,
    icon_size: Option<Vec2>,
}

impl IconButton {
    pub fn new(image_source: impl Into<ImageSource<'static>>) -> Self {
        IconButton {
            image_source: image_source.into(),
            tint: theme::color::ICON,
            active_state: IconButtonActiveState::new(theme::color::ICON_ACTIVE),
            active: false,
            size: Vec2::new(24.0, 24.0),
            icon_size: None,
        }
    }

    pub fn tint_active(mut self, color: Color32) -> Self {
        self.active_state.tint = color;
        self
    }

    pub fn icon_active(mut self, icon: impl Into<ImageSource<'static>>) -> Self {
        self.active_state.icon = Some(icon.into());
        self
    }

    pub fn active_state(mut self, active_state: IconButtonActiveState) -> Self {
        self.active_state = active_state;
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn tint(mut self, color: Color32) -> Self {
        self.tint = color;
        self
    }

    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    pub fn icon_size(mut self, size: Vec2) -> Self {
        self.icon_size = Some(size);
        self
    }

    fn color(&self) -> Color32 {
        if self.active {
            self.active_state.tint
        } else {
            self.tint
        }
    }

    fn image_source(&self) -> &ImageSource<'static> {
        if self.active {
            self.active_state
                .icon
                .as_ref()
                .unwrap_or(&self.image_source)
        } else {
            &self.image_source
        }
    }
}

impl Widget for IconButton {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(self.size, Sense::click());
        if response.hovered() {
            dep_mut!(CursorManager, |cursor_manager| cursor_manager
                .set_cursor(egui::CursorIcon::PointingHand))
        }

        let visuals = ui.style().interact_selectable(&response, self.active);
        if self.active
            || response.hovered()
            || response.has_focus()
            || response.is_pointer_button_down_on()
        {
            ui.painter().rect(
                rect,
                visuals.corner_radius,
                visuals.weak_bg_fill,
                if response.has_focus() {
                    egui::Stroke::new(1.0, theme::color::BLUE_SOFT)
                } else {
                    visuals.bg_stroke
                },
                egui::StrokeKind::Inside,
            );
        }

        let color = if response.hovered() && !self.active {
            theme::color::WHITE
        } else {
            self.color()
        };
        let icon_size = self
            .icon_size
            .unwrap_or(self.size - Vec2::splat(6.0))
            .min(self.size);

        Image::new(self.image_source().clone())
            .fit_to_exact_size(icon_size)
            .tint(color)
            .paint_at(ui, Rect::from_center_size(rect.center(), icon_size));

        response
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;

    fn source(uri: &'static str) -> ImageSource<'static> {
        ImageSource::Uri(Cow::Borrowed(uri))
    }

    #[test]
    fn active_state_without_an_icon_tints_the_main_icon() {
        let button = IconButton::new(source("base"))
            .active(true)
            .active_state(IconButtonActiveState::new(Color32::RED));

        assert_eq!(button.image_source().uri(), Some("base"));
        assert_eq!(button.color(), Color32::RED);
    }

    #[test]
    fn active_state_can_replace_the_main_icon() {
        let button = IconButton::new(source("base"))
            .active(true)
            .active_state(IconButtonActiveState::new(Color32::GREEN).icon(source("active")));

        assert_eq!(button.image_source().uri(), Some("active"));
        assert_eq!(button.color(), Color32::GREEN);
    }

    #[test]
    fn active_icon_can_be_set_directly() {
        let button = IconButton::new(source("base"))
            .active(true)
            .icon_active(source("active"));

        assert_eq!(button.image_source().uri(), Some("active"));
        assert_eq!(button.color(), theme::color::ICON_ACTIVE);
    }
}
