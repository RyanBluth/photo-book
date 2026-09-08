use egui::{
    Color32, Context, CornerRadius, Frame, Margin, Response, RichText, Shadow, Stroke, Ui,
    WidgetText, vec2,
};

use super::color;

pub const DIALOG_MIN_WIDTH: f32 = 400.0;
pub(crate) const CONTROL_HEIGHT: f32 = 28.0;
pub(crate) const TEXT_EDIT_MARGIN: Margin = Margin::symmetric(10, 6);

/// Apply the application-wide egui styling.
pub fn apply(ctx: &Context) {
    // Keep custom-painted chrome and egui widgets on the same dark palette,
    // independently of the operating system's appearance preference.
    ctx.set_theme(egui::Theme::Dark);
    ctx.global_style_mut(|style| {
        style.text_styles.extend([
            (egui::TextStyle::Small, egui::FontId::proportional(11.0)),
            (egui::TextStyle::Body, egui::FontId::proportional(13.0)),
            (egui::TextStyle::Button, egui::FontId::proportional(13.0)),
            (egui::TextStyle::Heading, egui::FontId::proportional(18.0)),
            (egui::TextStyle::Monospace, egui::FontId::monospace(12.0)),
        ]);
        style.spacing.item_spacing = vec2(8.0, 6.0);
        style.spacing.button_padding = vec2(10.0, 5.0);
        style.spacing.interact_size.y = CONTROL_HEIGHT;
        style.spacing.window_margin = Margin::same(16);
        style.spacing.menu_margin = Margin::same(6);
        style.spacing.indent = 16.0;
        style.spacing.slider_rail_height = 4.0;
        style.spacing.scroll.bar_width = 8.0;
        style.spacing.scroll.floating_width = 3.0;
        style.animation_time = 0.12;

        let visuals = &mut style.visuals;
        visuals.panel_fill = color::SIDE_PANEL_BACKGROUND;
        visuals.window_fill = color::SURFACE_DARK;
        visuals.extreme_bg_color = color::SURFACE_XX_DARK;
        visuals.faint_bg_color = color::SURFACE_DARK;
        visuals.code_bg_color = color::SURFACE_XX_DARK;
        visuals.weak_text_color = Some(color::TEXT_MUTED);
        visuals.hyperlink_color = color::BLUE_SOFT;
        visuals.warn_fg_color = color::WARNING;
        visuals.error_fg_color = color::ERROR;
        visuals.window_stroke = Stroke::new(1.0, color::BORDER);
        visuals.window_corner_radius = CornerRadius::same(6);
        visuals.menu_corner_radius = CornerRadius::same(4);
        visuals.window_shadow = Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(110),
        };
        visuals.popup_shadow = Shadow {
            offset: [0, 4],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(100),
        };
        visuals.selection.bg_fill = color::ACCENT_MUTED;
        visuals.selection.stroke = Stroke::new(1.0, color::WHITE);
        visuals.text_edit_bg_color = Some(color::SURFACE_XX_DARK);
        visuals.text_cursor.stroke = Stroke::new(1.5, color::BLUE_SOFT);
        visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);

        let widgets = &mut visuals.widgets;
        widgets.noninteractive.bg_fill = color::SURFACE_DARK;
        widgets.noninteractive.weak_bg_fill = color::SURFACE_DARK;
        widgets.noninteractive.bg_stroke = Stroke::new(1.0, color::BORDER);
        widgets.noninteractive.fg_stroke = Stroke::new(1.0, color::CONTROL_TEXT);

        widgets.inactive.bg_fill = color::SURFACE_STRONG;
        widgets.inactive.weak_bg_fill = color::SURFACE;
        widgets.inactive.bg_stroke = Stroke::new(1.0, color::BORDER);
        widgets.inactive.fg_stroke = Stroke::new(1.0, color::CONTROL_TEXT);

        widgets.hovered.bg_fill = color::SURFACE_EMPHASIS;
        widgets.hovered.weak_bg_fill = color::SURFACE_MUTED;
        widgets.hovered.bg_stroke = Stroke::new(1.0, color::SURFACE_STRONG);
        widgets.hovered.fg_stroke = Stroke::new(1.0, color::WHITE);

        widgets.active.bg_fill = color::BLUE_SOFT;
        widgets.active.weak_bg_fill = color::ACCENT_MUTED;
        widgets.active.bg_stroke = Stroke::new(1.0, color::BLUE_SOFT);
        widgets.active.fg_stroke = Stroke::new(1.0, color::WHITE);

        widgets.open.bg_fill = color::SURFACE_STRONG;
        widgets.open.weak_bg_fill = color::SURFACE_MUTED;
        widgets.open.bg_stroke = Stroke::new(1.0, color::BORDER);
        widgets.open.fg_stroke = Stroke::new(1.0, color::WHITE);

        for visuals in [
            &mut widgets.noninteractive,
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
            &mut widgets.open,
        ] {
            visuals.corner_radius = CornerRadius::same(3);
            visuals.expansion = 0.0;
        }
    });
}

pub fn dialog_frame() -> Frame {
    Frame::new()
        .inner_margin(20)
        .fill(color::SURFACE_X_DARK)
        .stroke(Stroke::new(1.0, color::BORDER))
        .corner_radius(6)
        .shadow(Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(140),
        })
}

pub fn dialog_title(ui: &mut Ui, title: impl Into<String>) {
    ui.label(
        RichText::new(title.into())
            .size(20.0)
            .strong()
            .color(color::WHITE),
    );
}

pub fn primary_button(ui: &mut Ui, text: impl Into<WidgetText>) -> Response {
    let text = text.into();
    ui.scope(|ui| {
        let widgets = &mut ui.style_mut().visuals.widgets;
        widgets.inactive.weak_bg_fill = color::ACCENT;
        widgets.inactive.bg_stroke = Stroke::new(1.0, color::ACCENT);
        widgets.inactive.fg_stroke = Stroke::new(1.0, color::WHITE);
        widgets.hovered.weak_bg_fill = color::ACCENT_HOVER;
        widgets.hovered.bg_stroke = Stroke::new(1.0, color::ACCENT_HOVER);
        widgets.hovered.fg_stroke = Stroke::new(1.0, color::WHITE);
        widgets.active.weak_bg_fill = color::ACCENT_PRESSED;
        widgets.active.bg_stroke = Stroke::new(1.0, color::ACCENT);
        widgets.active.fg_stroke = Stroke::new(1.0, color::WHITE);

        ui.button(text)
    })
    .inner
}

pub fn primary_button_enabled(ui: &mut Ui, enabled: bool, text: impl Into<WidgetText>) -> Response {
    let text = text.into();
    ui.add_enabled_ui(enabled, |ui| primary_button(ui, text))
        .inner
}
