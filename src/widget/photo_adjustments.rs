use egui::{
    CollapsingHeader, Color32, ComboBox, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
    Ui, Vec2, epaint::Mesh, pos2,
};

use crate::{
    model::photo_adjustments::{
        Curve, CurvePoint, LevelValues, LevelsAdjustments, LightAdjustments, PhotoAdjustments,
    },
    theme::color,
    widget::{
        adjustment_slider::AdjustmentSlider,
        edit_response::EditResponse,
        histogram::{BIN_COUNT, HistogramData},
    },
};

const MAX_CURVE_POINTS: usize = 16;

#[derive(Debug, Clone)]
pub struct PhotoAdjustmentsState {
    pub curve_channel: CurveChannel,
    pub levels_channel: LevelsChannel,
}

impl PhotoAdjustmentsState {
    pub fn new() -> Self {
        Self {
            curve_channel: CurveChannel::Rgb,
            levels_channel: LevelsChannel::Rgb,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveChannel {
    Rgb,
    Red,
    Green,
    Blue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelsChannel {
    Luminance,
    Rgb,
    Red,
    Green,
    Blue,
}

impl LevelsChannel {
    fn label(self) -> &'static str {
        match self {
            Self::Luminance => "Luminance",
            Self::Rgb => "RGB",
            Self::Red => "Red",
            Self::Green => "Green",
            Self::Blue => "Blue",
        }
    }
}

impl CurveChannel {
    fn label(self) -> &'static str {
        match self {
            Self::Rgb => "RGB",
            Self::Red => "Red",
            Self::Green => "Green",
            Self::Blue => "Blue",
        }
    }

    fn color(self) -> Color32 {
        match self {
            Self::Rgb => color::SURFACE_EMPHASIS,
            Self::Red => color::ERROR.linear_multiply(0.72),
            Self::Green => color::SUCCESS.linear_multiply(0.58),
            Self::Blue => color::BLUE_SOFT,
        }
    }
}

pub struct PhotoAdjustmentsEditor<'a> {
    adjustments: &'a mut PhotoAdjustments,
    state: &'a mut PhotoAdjustmentsState,
    histogram: Option<&'a HistogramData>,
}

impl<'a> PhotoAdjustmentsEditor<'a> {
    pub fn new(
        adjustments: &'a mut PhotoAdjustments,
        state: &'a mut PhotoAdjustmentsState,
    ) -> Self {
        Self {
            adjustments,
            state,
            histogram: None,
        }
    }

    pub fn histogram(mut self, histogram: Option<&'a HistogramData>) -> Self {
        self.histogram = histogram;
        self
    }

    pub fn show(mut self, ui: &mut Ui) -> EditResponse {
        let mut response = EditResponse::none();

        ui.horizontal(|ui| {
            ui.label(RichText::new("Adjust").small().strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        !self.adjustments.is_identity(),
                        egui::Button::new(RichText::new("Reset").size(12.0)).frame(false),
                    )
                    .clicked()
                {
                    *self.adjustments = PhotoAdjustments::default();
                    response |= EditResponse::discrete(true);
                }
            });
        });

        response |= self.show_light(ui);
        response |= self.show_color(ui);
        response |= self.show_curve(ui);
        response |= self.show_black_white(ui);
        response |= self.show_detail(ui);
        response |= self.show_levels(ui);

        response
    }

    fn show_light(&mut self, ui: &mut Ui) -> EditResponse {
        let mut response = EditResponse::none();
        CollapsingHeader::new(section_title(
            "Light",
            self.adjustments.light != Default::default(),
        ))
        .id_salt("photo_adjustments_light")
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        self.histogram.is_some(),
                        egui::Button::new(RichText::new("Auto").size(12.0)),
                    )
                    .clicked()
                    && let Some(histogram) = self.histogram
                {
                    self.adjustments.light = auto_light(histogram);
                    response |= EditResponse::discrete(true);
                }

                response |= EditResponse::discrete(reset_section_button(
                    ui,
                    self.adjustments.light != Default::default(),
                    || {
                        self.adjustments.light = Default::default();
                    },
                ));
            });
            response |= adjustment_slider(ui, "Brilliance", &mut self.adjustments.light.brilliance);
            response |= adjustment_slider(ui, "Exposure", &mut self.adjustments.light.exposure);
            response |= adjustment_slider(ui, "Highlights", &mut self.adjustments.light.highlights);
            response |= adjustment_slider(ui, "Shadows", &mut self.adjustments.light.shadows);
            response |= adjustment_slider(ui, "Brightness", &mut self.adjustments.light.brightness);
            response |= adjustment_slider(ui, "Contrast", &mut self.adjustments.light.contrast);
            response |=
                adjustment_slider(ui, "Black Point", &mut self.adjustments.light.black_point);
        });
        response
    }

    fn show_color(&mut self, ui: &mut Ui) -> EditResponse {
        let mut response = EditResponse::none();
        let is_active = self.adjustments.color != Default::default()
            || self.adjustments.white_balance != Default::default();
        CollapsingHeader::new(section_title("Color", is_active))
            .id_salt("photo_adjustments_color")
            .default_open(false)
            .show(ui, |ui| {
                response |= EditResponse::discrete(reset_section_button(ui, is_active, || {
                    self.adjustments.color = Default::default();
                    self.adjustments.white_balance = Default::default();
                }));
                response |=
                    adjustment_slider(ui, "Saturation", &mut self.adjustments.color.saturation);
                response |= adjustment_slider(ui, "Vibrance", &mut self.adjustments.color.vibrance);
                response |= adjustment_slider(ui, "Cast", &mut self.adjustments.color.cast);
            });
        response
    }

    fn show_black_white(&mut self, ui: &mut Ui) -> EditResponse {
        let mut response = EditResponse::none();
        let is_active = self.adjustments.black_white.intensity != 0.0
            || self.adjustments.black_white.neutrals != 0.0
            || self.adjustments.black_white.tone != 0.0
            || self.adjustments.black_white.grain != 0.0;
        CollapsingHeader::new(section_title("Black & White", is_active))
            .id_salt("photo_adjustments_black_white")
            .default_open(false)
            .show(ui, |ui| {
                response |= EditResponse::discrete(reset_section_button(ui, is_active, || {
                    self.adjustments.black_white.intensity = 0.0;
                    self.adjustments.black_white.neutrals = 0.0;
                    self.adjustments.black_white.tone = 0.0;
                    self.adjustments.black_white.grain = 0.0;
                }));
                response |=
                    positive_slider(ui, "Intensity", &mut self.adjustments.black_white.intensity);
                response |=
                    adjustment_slider(ui, "Neutrals", &mut self.adjustments.black_white.neutrals);
                response |= adjustment_slider(ui, "Tone", &mut self.adjustments.black_white.tone);
                response |= positive_slider(ui, "Grain", &mut self.adjustments.black_white.grain);
            });
        response
    }

    fn show_curve(&mut self, ui: &mut Ui) -> EditResponse {
        let mut response = EditResponse::none();
        CollapsingHeader::new(section_title(
            "Curve",
            self.adjustments.curves != Default::default(),
        ))
        .id_salt("photo_adjustments_curve")
        .show(ui, |ui| {
            let reset_enabled = self.selected_curve() != &Curve::default();
            if curve_toolbar(ui, &mut self.state.curve_channel, reset_enabled).reset_clicked {
                *self.selected_curve_mut() = Curve::default();
                response |= EditResponse::discrete(true);
            }

            ui.add_space(8.0);
            let channel = self.state.curve_channel;
            let histogram = self.histogram;
            let curve = self.selected_curve_mut();
            response |= curve_editor(ui, curve, histogram, channel);
        });
        response
    }

    fn show_levels(&mut self, ui: &mut Ui) -> EditResponse {
        let mut response = EditResponse::none();
        CollapsingHeader::new(section_title(
            "Levels",
            self.adjustments.levels != Default::default(),
        ))
        .id_salt("photo_adjustments_levels")
        .default_open(false)
        .show(ui, |ui| {
            response |= EditResponse::discrete(reset_section_button(
                ui,
                self.adjustments.levels != Default::default(),
                || {
                    self.adjustments.levels = Default::default();
                },
            ));
            response |= levels_editor(
                ui,
                &mut self.adjustments.levels,
                self.histogram,
                &mut self.state.levels_channel,
            );
        });
        response
    }

    fn show_detail(&mut self, ui: &mut Ui) -> EditResponse {
        let mut response = EditResponse::none();
        CollapsingHeader::new(section_title(
            "Detail",
            self.adjustments.definition != Default::default(),
        ))
        .id_salt("photo_adjustments_detail")
        .default_open(false)
        .show(ui, |ui| {
            response |= EditResponse::discrete(reset_section_button(
                ui,
                self.adjustments.definition != Default::default(),
                || {
                    self.adjustments.definition = Default::default();
                },
            ));
            response |= positive_slider(ui, "Sharpening", &mut self.adjustments.definition.amount);
        });
        response
    }

    fn selected_curve(&self) -> &Curve {
        match self.state.curve_channel {
            CurveChannel::Rgb => &self.adjustments.curves.rgb,
            CurveChannel::Red => &self.adjustments.curves.red,
            CurveChannel::Green => &self.adjustments.curves.green,
            CurveChannel::Blue => &self.adjustments.curves.blue,
        }
    }

    fn selected_curve_mut(&mut self) -> &mut Curve {
        match self.state.curve_channel {
            CurveChannel::Rgb => &mut self.adjustments.curves.rgb,
            CurveChannel::Red => &mut self.adjustments.curves.red,
            CurveChannel::Green => &mut self.adjustments.curves.green,
            CurveChannel::Blue => &mut self.adjustments.curves.blue,
        }
    }
}

fn section_title(title: &str, active: bool) -> String {
    if active {
        format!("● {title}")
    } else {
        title.to_string()
    }
}

fn reset_section_button(ui: &mut Ui, active: bool, reset: impl FnOnce()) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(
                    active,
                    egui::Button::new(RichText::new("Reset").size(12.0)).frame(false),
                )
                .clicked()
            {
                reset();
                clicked = true;
            }
        });
    });
    clicked
}

#[derive(Debug, Clone, Copy, Default)]
struct CurveToolbarResponse {
    reset_clicked: bool,
}

fn curve_toolbar(
    ui: &mut Ui,
    current: &mut CurveChannel,
    reset_enabled: bool,
) -> CurveToolbarResponse {
    let mut reset_clicked = false;
    ui.horizontal(|ui| {
        let dropdown_width = (ui.available_width() - 62.0).max(120.0);
        ComboBox::from_id_salt("curves_channel")
            .selected_text(RichText::new(current.label()).size(14.0).strong())
            .width(dropdown_width)
            .show_ui(ui, |ui| {
                ui.selectable_value(current, CurveChannel::Rgb, CurveChannel::Rgb.label());
                ui.separator();
                ui.selectable_value(current, CurveChannel::Red, CurveChannel::Red.label());
                ui.selectable_value(current, CurveChannel::Green, CurveChannel::Green.label());
                ui.selectable_value(current, CurveChannel::Blue, CurveChannel::Blue.label());
            });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(
                    reset_enabled,
                    egui::Button::new(RichText::new("Reset").size(12.0)).frame(false),
                )
                .clicked()
            {
                reset_clicked = true;
            }
        });
    });

    CurveToolbarResponse { reset_clicked }
}

fn auto_light(histogram: &HistogramData) -> LightAdjustments {
    let luma = histogram.luma();
    let total = luma.iter().sum::<f32>().max(f32::EPSILON);
    let center = luma
        .iter()
        .enumerate()
        .map(|(index, value)| index as f32 / (BIN_COUNT - 1) as f32 * *value)
        .sum::<f32>()
        / total;
    let shadow_weight = luma.iter().take(10).sum::<f32>() / total;
    let highlight_weight = luma.iter().rev().take(10).sum::<f32>() / total;

    LightAdjustments {
        exposure: ((0.5 - center) * 1.15).clamp(-0.75, 0.75),
        contrast: 0.08,
        highlights: (-highlight_weight * 1.2).clamp(-0.45, 0.0),
        shadows: (shadow_weight * 1.2).clamp(0.0, 0.45),
        black_point: 0.0,
        brightness: 0.0,
        brilliance: 0.0,
    }
}

fn adjustment_slider(ui: &mut Ui, label: &str, value: &mut f32) -> EditResponse {
    slider(ui, label, value, -1.0..=1.0, 0.0)
}

fn positive_slider(ui: &mut Ui, label: &str, value: &mut f32) -> EditResponse {
    slider(ui, label, value, 0.0..=1.0, 0.0)
}

fn slider(
    ui: &mut Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    default: f32,
) -> EditResponse {
    let response = AdjustmentSlider::new(label, value)
        .range(range)
        .default(default)
        .show(ui);
    EditResponse::drag(&response)
}

fn levels_editor(
    ui: &mut Ui,
    levels: &mut LevelsAdjustments,
    histogram: Option<&HistogramData>,
    channel: &mut LevelsChannel,
) -> EditResponse {
    ComboBox::from_id_salt("levels_channel")
        .selected_text(RichText::new(channel.label()).size(14.0).strong())
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            ui.selectable_value(channel, LevelsChannel::Luminance, "Luminance");
            ui.selectable_value(channel, LevelsChannel::Rgb, "RGB");
            ui.separator();
            ui.selectable_value(channel, LevelsChannel::Red, "Red");
            ui.selectable_value(channel, LevelsChannel::Green, "Green");
            ui.selectable_value(channel, LevelsChannel::Blue, "Blue");
        });

    ui.add_space(8.0);

    let selected_channel = *channel;
    let values = selected_level_values_mut(levels, selected_channel);
    normalize_levels(values);

    let graph_size = Vec2::new(ui.available_width(), 154.0);
    let (response, painter) = ui.allocate_painter(graph_size, Sense::click_and_drag());
    let plot_rect = Rect::from_min_max(
        response.rect.min + Vec2::new(2.0, 14.0),
        response.rect.max - Vec2::new(2.0, 20.0),
    );
    let mut edit_response = EditResponse::none();

    if ui.is_rect_visible(response.rect) {
        painter.rect_filled(plot_rect, 6.0, color::SURFACE_DARK);
        paint_levels_grid(&painter, plot_rect);

        if let Some(histogram) = histogram {
            paint_levels_histogram(&painter, plot_rect, histogram, *channel);
        } else {
            painter.text(
                plot_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Histogram unavailable",
                FontId::proportional(12.0),
                color::SURFACE_STRONG,
            );
        }

        painter.rect_stroke(
            plot_rect,
            6.0,
            Stroke::new(1.0, color::SURFACE_MUTED.linear_multiply(0.7)),
            StrokeKind::Inside,
        );
    }

    let handles = [
        ("black", values.black, color::SURFACE_STRONG),
        ("mid", values.mid, color::SURFACE_STRONG),
        ("white", values.white, color::SURFACE_STRONG),
    ];

    for (name, value, handle_color) in handles {
        let x = plot_rect.left() + value * plot_rect.width();
        paint_levels_top_marker(&painter, plot_rect, x, handle_color);
        paint_levels_bottom_marker(&painter, plot_rect, x, handle_color);

        let handle_rect =
            Rect::from_center_size(pos2(x, plot_rect.bottom() + 7.0), Vec2::new(22.0, 22.0));
        let handle_response = ui.interact(handle_rect, response.id.with(name), Sense::drag());
        if handle_response.dragged()
            && let Some(pointer_pos) = handle_response.interact_pointer_pos()
        {
            let next = ((pointer_pos.x - plot_rect.left()) / plot_rect.width()).clamp(0.0, 1.0);
            match name {
                "black" => values.black = next.min(values.mid - 0.01).clamp(0.0, 0.98),
                "mid" => values.mid = next.clamp(values.black + 0.01, values.white - 0.01),
                "white" => values.white = next.max(values.mid + 0.01).clamp(0.02, 1.0),
                _ => {}
            }
            let mut response = handle_response;
            response.mark_changed();
            edit_response |= EditResponse::drag(&response);
        }
    }

    if response.double_clicked() {
        *values = LevelValues::default();
        edit_response |= EditResponse::discrete(true);
    }

    normalize_levels(values);
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Levels editor"));

    edit_response
}

fn selected_level_values_mut(
    levels: &mut LevelsAdjustments,
    channel: LevelsChannel,
) -> &mut LevelValues {
    match channel {
        LevelsChannel::Luminance => &mut levels.luminance,
        LevelsChannel::Rgb => &mut levels.rgb,
        LevelsChannel::Red => &mut levels.red,
        LevelsChannel::Green => &mut levels.green,
        LevelsChannel::Blue => &mut levels.blue,
    }
}

fn normalize_levels(levels: &mut LevelValues) {
    levels.black = levels.black.clamp(0.0, 0.98);
    levels.white = levels.white.clamp(0.02, 1.0);
    levels.mid = levels.mid.clamp(0.01, 0.99);

    if levels.black >= levels.mid {
        levels.black = (levels.mid - 0.01).max(0.0);
    }
    if levels.white <= levels.mid {
        levels.white = (levels.mid + 0.01).min(1.0);
    }
}

fn paint_levels_grid(painter: &egui::Painter, rect: Rect) {
    for index in 1..4 {
        let x = rect.left() + rect.width() * index as f32 / 4.0;
        painter.line_segment(
            [pos2(x, rect.top()), pos2(x, rect.bottom())],
            Stroke::new(1.0, color::SURFACE_MUTED.linear_multiply(0.8)),
        );
    }
}

fn paint_levels_histogram(
    painter: &egui::Painter,
    rect: Rect,
    histogram: &HistogramData,
    channel: LevelsChannel,
) {
    let clipped = painter.with_clip_rect(rect);
    match channel {
        LevelsChannel::Luminance => paint_histogram_fill(
            &clipped,
            rect,
            histogram.luma(),
            Color32::from_rgba_unmultiplied(210, 210, 210, 82),
        ),
        LevelsChannel::Rgb => {
            paint_histogram_fill(
                &clipped,
                rect,
                histogram.luma(),
                Color32::from_rgba_unmultiplied(220, 220, 220, 70),
            );
            paint_histogram_line(
                &clipped,
                rect,
                histogram.red(),
                Color32::from_rgba_unmultiplied(255, 72, 64, 220),
            );
            paint_histogram_line(
                &clipped,
                rect,
                histogram.green(),
                Color32::from_rgba_unmultiplied(54, 220, 76, 210),
            );
            paint_histogram_line(
                &clipped,
                rect,
                histogram.blue(),
                Color32::from_rgba_unmultiplied(20, 148, 255, 230),
            );
        }
        LevelsChannel::Red => paint_histogram_fill(
            &clipped,
            rect,
            histogram.red(),
            Color32::from_rgba_unmultiplied(255, 72, 64, 100),
        ),
        LevelsChannel::Green => paint_histogram_fill(
            &clipped,
            rect,
            histogram.green(),
            Color32::from_rgba_unmultiplied(54, 220, 76, 90),
        ),
        LevelsChannel::Blue => paint_histogram_fill(
            &clipped,
            rect,
            histogram.blue(),
            Color32::from_rgba_unmultiplied(20, 148, 255, 100),
        ),
    }
}

fn paint_levels_top_marker(painter: &egui::Painter, rect: Rect, x: f32, color: Color32) {
    let points = vec![
        pos2(x - 6.0, rect.top() - 4.0),
        pos2(x + 6.0, rect.top() - 4.0),
        pos2(x, rect.top() + 7.0),
    ];
    painter.add(egui::Shape::convex_polygon(
        points,
        color.linear_multiply(0.55),
        Stroke::NONE,
    ));
}

fn paint_levels_bottom_marker(painter: &egui::Painter, rect: Rect, x: f32, color: Color32) {
    let center = pos2(x, rect.bottom() + 7.0);
    painter.circle_filled(center, 7.0, color.linear_multiply(0.58));
    painter.circle_stroke(center, 7.0, Stroke::new(1.0, color::SURFACE_X_DARK));
}

fn curve_editor(
    ui: &mut Ui,
    curve: &mut Curve,
    histogram: Option<&HistogramData>,
    channel: CurveChannel,
) -> EditResponse {
    normalize_curve_points(curve);

    let mut edit_response = EditResponse::none();
    let available_width = ui.available_width();
    let graph_size = available_width.clamp(180.0, 440.0);
    let left_padding = ((available_width - graph_size) * 0.5).max(0.0);

    ui.horizontal(|ui| {
        ui.add_space(left_padding);

        let (response, painter) =
            ui.allocate_painter(Vec2::splat(graph_size), Sense::click_and_drag());
        let plot_rect = response.rect.shrink(6.0);

        if ui.is_rect_visible(response.rect) {
            painter.rect_filled(response.rect, 7.0, color::SURFACE_X_DARK);
            paint_curve_grid(&painter, plot_rect);

            if let Some(histogram) = histogram {
                paint_curve_histogram(&painter, plot_rect, histogram, channel);
            } else {
                painter.text(
                    plot_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Histogram unavailable",
                    FontId::proportional(12.0),
                    color::SURFACE_STRONG,
                );
            }

            paint_identity_curve(&painter, plot_rect);
        }

        let mut point_interacted = false;
        let original_points = curve.points.clone();
        let last_index = curve.points.len().saturating_sub(1);

        for index in 0..curve.points.len() {
            let screen_pos = curve_point_to_screen(plot_rect, &curve.points[index]);
            let point_rect = Rect::from_center_size(screen_pos, Vec2::splat(14.0));
            let point_id = response.id.with(("curve_point", index));
            let point_response = ui.interact(point_rect, point_id, Sense::click_and_drag());

            if point_response.dragged()
                && let Some(pointer_pos) = point_response.interact_pointer_pos()
            {
                let mut next_point = screen_to_curve_point(plot_rect, pointer_pos);
                if index == 0 {
                    next_point.x = 0.0;
                } else if index == last_index {
                    next_point.x = 1.0;
                } else {
                    let min_x = original_points[index - 1].x + 0.01;
                    let max_x = original_points[index + 1].x - 0.01;
                    next_point.x = next_point.x.clamp(min_x, max_x);
                }

                curve.points[index] = next_point;
                let mut response = point_response.clone();
                response.mark_changed();
                edit_response |= EditResponse::drag(&response);
            }

            if (point_response.double_clicked() || point_response.secondary_clicked())
                && index != 0
                && index != last_index
            {
                curve.points.remove(index);
                edit_response |= EditResponse::discrete(true);
                point_interacted = true;
                break;
            }

            point_interacted |=
                point_response.hovered() || point_response.dragged() || point_response.clicked();
        }

        if response.clicked()
            && !point_interacted
            && curve.points.len() < MAX_CURVE_POINTS
            && let Some(pointer_pos) = response.interact_pointer_pos()
        {
            let point = screen_to_curve_point(plot_rect, pointer_pos);
            curve.points.push(point);
            edit_response |= EditResponse::discrete(true);
        }

        if edit_response.changed {
            normalize_curve_points(curve);
        }

        paint_curve_line(&painter, plot_rect, curve, channel.color());
        paint_curve_points(&painter, plot_rect, curve, &response);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Curve editor")
        });
    });

    edit_response
}

fn normalize_curve_points(curve: &mut Curve) {
    curve
        .points
        .retain(|point| point.x.is_finite() && point.y.is_finite());
    if curve.points.is_empty() {
        curve.points = Curve::default().points;
    }

    curve.points.iter_mut().for_each(|point| {
        point.x = point.x.clamp(0.0, 1.0);
        point.y = point.y.clamp(0.0, 1.0);
    });
    curve.points.sort_by(|left, right| {
        left.x
            .partial_cmp(&right.x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if curve.points.first().is_none_or(|point| point.x > 0.0) {
        curve.points.insert(0, CurvePoint { x: 0.0, y: 0.0 });
    }
    if curve.points.last().is_none_or(|point| point.x < 1.0) {
        curve.points.push(CurvePoint { x: 1.0, y: 1.0 });
    }

    if let Some(first) = curve.points.first_mut() {
        first.x = 0.0;
    }
    if let Some(last) = curve.points.last_mut() {
        last.x = 1.0;
    }

    curve.points.dedup_by(|right, left| {
        if (left.x - right.x).abs() < 0.001 {
            right.x = left.x;
            true
        } else {
            false
        }
    });
}

fn paint_curve_grid(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(rect, 4.0, color::SURFACE_DARK);

    for index in 1..4 {
        let x = rect.left() + rect.width() * index as f32 / 4.0;
        painter.line_segment(
            [pos2(x, rect.top()), pos2(x, rect.bottom())],
            Stroke::new(1.0, color::SURFACE.linear_multiply(0.64)),
        );
        let y = rect.top() + rect.height() * index as f32 / 4.0;
        painter.line_segment(
            [pos2(rect.left(), y), pos2(rect.right(), y)],
            Stroke::new(1.0, color::SURFACE.linear_multiply(0.64)),
        );
    }

    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, color::SURFACE_MUTED.linear_multiply(0.68)),
        StrokeKind::Inside,
    );
}

fn paint_identity_curve(painter: &egui::Painter, rect: Rect) {
    painter.line_segment(
        [rect.left_bottom(), rect.right_top()],
        Stroke::new(1.15, color::SURFACE_EMPHASIS.linear_multiply(0.58)),
    );
}

fn paint_curve_histogram(
    painter: &egui::Painter,
    rect: Rect,
    histogram: &HistogramData,
    channel: CurveChannel,
) {
    let clipped = painter.with_clip_rect(rect);
    match channel {
        CurveChannel::Rgb => {
            paint_histogram_fill(
                &clipped,
                rect,
                histogram.luma(),
                Color32::from_rgba_unmultiplied(210, 210, 210, 48),
            );
            paint_histogram_fill(
                &clipped,
                rect,
                histogram.red(),
                Color32::from_rgba_unmultiplied(255, 56, 50, 34),
            );
            paint_histogram_fill(
                &clipped,
                rect,
                histogram.green(),
                Color32::from_rgba_unmultiplied(58, 222, 74, 30),
            );
            paint_histogram_fill(
                &clipped,
                rect,
                histogram.blue(),
                Color32::from_rgba_unmultiplied(61, 148, 255, 38),
            );
        }
        CurveChannel::Red => paint_histogram_fill(
            &clipped,
            rect,
            histogram.red(),
            Color32::from_rgba_unmultiplied(255, 56, 50, 58),
        ),
        CurveChannel::Green => paint_histogram_fill(
            &clipped,
            rect,
            histogram.green(),
            Color32::from_rgba_unmultiplied(58, 222, 74, 48),
        ),
        CurveChannel::Blue => paint_histogram_fill(
            &clipped,
            rect,
            histogram.blue(),
            Color32::from_rgba_unmultiplied(61, 148, 255, 58),
        ),
    }
}

fn paint_histogram_fill(
    painter: &egui::Painter,
    rect: Rect,
    values: &[f32; BIN_COUNT],
    color: Color32,
) {
    let points = histogram_points(rect, values);
    let mut mesh = Mesh::default();

    for window in points.windows(2) {
        let left_top = window[0];
        let right_top = window[1];
        let left_bottom = pos2(left_top.x, rect.bottom());
        let right_bottom = pos2(right_top.x, rect.bottom());
        let index = mesh.vertices.len() as u32;

        mesh.colored_vertex(left_bottom, color);
        mesh.colored_vertex(left_top, color);
        mesh.colored_vertex(right_top, color);
        mesh.colored_vertex(right_bottom, color);
        mesh.add_triangle(index, index + 1, index + 2);
        mesh.add_triangle(index, index + 2, index + 3);
    }

    painter.add(egui::Shape::mesh(mesh));
}

fn paint_histogram_line(
    painter: &egui::Painter,
    rect: Rect,
    values: &[f32; BIN_COUNT],
    color: Color32,
) {
    painter.add(egui::Shape::line(
        histogram_points(rect, values),
        Stroke::new(1.35, color),
    ));
}

fn histogram_points(rect: Rect, values: &[f32; BIN_COUNT]) -> Vec<Pos2> {
    let bin_width = rect.width() / (BIN_COUNT - 1) as f32;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            pos2(
                rect.left() + index as f32 * bin_width,
                rect.bottom() - value.clamp(0.0, 1.0) * rect.height(),
            )
        })
        .collect()
}

fn paint_curve_line(painter: &egui::Painter, rect: Rect, curve: &Curve, curve_color: Color32) {
    let mut points = curve.points.clone();
    points.sort_by(|left, right| {
        left.x
            .partial_cmp(&right.x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if points.len() < 2 {
        return;
    }

    let screen_points = points
        .iter()
        .map(|point| curve_point_to_screen(rect, point))
        .collect();
    painter.add(egui::Shape::line(
        screen_points,
        Stroke::new(2.0, curve_color),
    ));
}

fn paint_curve_points(
    painter: &egui::Painter,
    rect: Rect,
    curve: &Curve,
    response: &egui::Response,
) {
    let hover_pos = response.hover_pos();
    for point in &curve.points {
        let point_pos = curve_point_to_screen(rect, point);
        let hovered = hover_pos.is_some_and(|pos| pos.distance(point_pos) <= 8.0);
        let radius = if hovered { 4.2 } else { 3.4 };
        painter.circle_filled(point_pos, radius, color::SURFACE_X_DARK);
        painter.circle_stroke(
            point_pos,
            if hovered { 6.4 } else { 5.2 },
            Stroke::new(if hovered { 1.8 } else { 1.35 }, color::SURFACE_EMPHASIS),
        );
    }
}

fn curve_point_to_screen(rect: Rect, point: &CurvePoint) -> Pos2 {
    pos2(
        rect.left() + point.x.clamp(0.0, 1.0) * rect.width(),
        rect.bottom() - point.y.clamp(0.0, 1.0) * rect.height(),
    )
}

fn screen_to_curve_point(rect: Rect, position: Pos2) -> CurvePoint {
    CurvePoint {
        x: ((position.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
        y: ((rect.bottom() - position.y) / rect.height()).clamp(0.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;

    #[test]
    fn photo_adjustments_editor_renders() {
        let mut adjustments = PhotoAdjustments::default();
        let mut state = PhotoAdjustmentsState::new();

        let mut harness = Harness::new_ui(|ui| {
            ui.set_width(320.0);
            let response = PhotoAdjustmentsEditor::new(&mut adjustments, &mut state).show(ui);

            assert!(!response.changed);
        });

        harness.run();
    }

    #[test]
    fn section_title_marks_active_sections() {
        assert_eq!(section_title("Light", false), "Light");
        assert_eq!(section_title("Light", true), "● Light");
    }
}
