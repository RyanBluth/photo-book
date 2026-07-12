use egui::{
    Color32, Context, CornerRadius, Frame, Response, RichText, Shadow, Stroke, Ui, WidgetText, vec2,
};

use super::color;

pub const DIALOG_MIN_WIDTH: f32 = 400.0;

/// Apply the application-wide egui styling.
pub fn apply(ctx: &Context) {
    ctx.global_style_mut(|style| {
        style.spacing.button_padding = vec2(12.0, 6.0);
        style.spacing.interact_size.y = 30.0;

        let widgets = &mut style.visuals.widgets;
        widgets.inactive.weak_bg_fill = color::SURFACE;
        widgets.inactive.bg_stroke = Stroke::new(1.0, color::SURFACE_MUTED);
        widgets.inactive.fg_stroke = Stroke::new(1.0, color::CONTROL_TEXT);

        widgets.hovered.weak_bg_fill = color::SURFACE_MUTED;
        widgets.hovered.bg_stroke = Stroke::new(1.0, color::SURFACE_STRONG);
        widgets.hovered.fg_stroke = Stroke::new(1.0, color::WHITE);

        widgets.active.weak_bg_fill = color::ACCENT_MUTED;
        widgets.active.bg_stroke = Stroke::new(1.0, color::ACCENT);
        widgets.active.fg_stroke = Stroke::new(1.0, color::WHITE);

        for visuals in [
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
            &mut widgets.open,
        ] {
            visuals.corner_radius = CornerRadius::same(6);
            visuals.expansion = 0.0;
        }

        style.visuals.selection.bg_fill = color::ACCENT;
        style.visuals.selection.stroke = Stroke::new(1.0, color::WHITE);
        style.visuals.text_edit_bg_color = Some(color::SURFACE_XX_DARK);
        style.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    });
}

pub fn dialog_frame() -> Frame {
    Frame::new()
        .inner_margin(20)
        .fill(color::SURFACE_X_DARK)
        .stroke(Stroke::new(1.0, color::SURFACE_MUTED))
        .corner_radius(10)
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
        widgets.hovered.weak_bg_fill = color::BLUE_SOFT;
        widgets.hovered.bg_stroke = Stroke::new(1.0, color::BLUE_SOFT);
        widgets.hovered.fg_stroke = Stroke::new(1.0, color::WHITE);
        widgets.active.weak_bg_fill = color::ACCENT_MUTED;
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
