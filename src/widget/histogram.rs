use egui::{
    Color32, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget, epaint::Mesh, pos2,
};
use image::{GenericImageView, Pixel};

use crate::model::photo_adjustments::PhotoAdjustments;
use crate::theme::color;

pub const BIN_COUNT: usize = 128;
const MAX_SAMPLES: u64 = 262_144;
const GRID_DIVISIONS: usize = 4;

#[derive(Debug, Clone)]
pub struct HistogramData {
    red: [f32; BIN_COUNT],
    green: [f32; BIN_COUNT],
    blue: [f32; BIN_COUNT],
    luma: [f32; BIN_COUNT],
}

impl HistogramData {
    pub(crate) fn from_image_with_adjustments(
        image: &image::DynamicImage,
        adjustments: &PhotoAdjustments,
    ) -> Self {
        if adjustments.is_identity() {
            return Self::from_image(image);
        }

        let image = image.clone();
        let adjusted = adjustments.apply_to_image(image);
        Self::from_image(&image::DynamicImage::ImageRgba8(adjusted))
    }

    pub(crate) fn from_image(image: &image::DynamicImage) -> Self {
        let (width, height) = image.dimensions();
        let stride = sample_stride(width, height);

        let mut red = [0_u32; BIN_COUNT];
        let mut green = [0_u32; BIN_COUNT];
        let mut blue = [0_u32; BIN_COUNT];
        let mut luma = [0_u32; BIN_COUNT];

        for y in (0..height).step_by(stride) {
            for x in (0..width).step_by(stride) {
                let pixel = image.get_pixel(x, y).to_rgb();
                let [r, g, b] = pixel.0;

                red[bin_index(r)] += 1;
                green[bin_index(g)] += 1;
                blue[bin_index(b)] += 1;
                luma[bin_index(luma_value(r, g, b))] += 1;
            }
        }

        let max_count = red
            .iter()
            .chain(green.iter())
            .chain(blue.iter())
            .chain(luma.iter())
            .copied()
            .max()
            .unwrap_or(1)
            .max(1);

        Self {
            red: normalize(red, max_count),
            green: normalize(green, max_count),
            blue: normalize(blue, max_count),
            luma: normalize(luma, max_count),
        }
    }

    pub fn red(&self) -> &[f32; BIN_COUNT] {
        &self.red
    }

    pub fn green(&self) -> &[f32; BIN_COUNT] {
        &self.green
    }

    pub fn blue(&self) -> &[f32; BIN_COUNT] {
        &self.blue
    }

    pub fn luma(&self) -> &[f32; BIN_COUNT] {
        &self.luma
    }
}

pub struct Histogram<'a> {
    data: Option<&'a HistogramData>,
    height: f32,
    loading: bool,
}

impl<'a> Histogram<'a> {
    pub fn new(data: &'a HistogramData) -> Self {
        Self {
            data: Some(data),
            height: 82.0,
            loading: false,
        }
    }

    pub fn unavailable() -> Self {
        Self {
            data: None,
            height: 82.0,
            loading: false,
        }
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }
}

impl Widget for Histogram<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let desired_size = Vec2::new(ui.available_width(), self.height);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::hover());

        if ui.is_rect_visible(rect) {
            paint_background(ui, rect);

            if let Some(data) = self.data {
                let plot_rect = rect.shrink2(Vec2::new(7.0, 6.0));
                paint_histogram(ui, plot_rect, data);
            }

            if self.loading {
                paint_loading_indicator(ui, rect);
            }
        }

        response
    }
}

fn sample_stride(width: u32, height: u32) -> usize {
    let pixels = u64::from(width) * u64::from(height);
    if pixels <= MAX_SAMPLES {
        return 1;
    }

    ((pixels as f64 / MAX_SAMPLES as f64).sqrt().ceil() as usize).max(1)
}

fn bin_index(value: u8) -> usize {
    (usize::from(value) * BIN_COUNT / 256).min(BIN_COUNT - 1)
}

fn luma_value(red: u8, green: u8, blue: u8) -> u8 {
    (0.2126 * f32::from(red) + 0.7152 * f32::from(green) + 0.0722 * f32::from(blue)).round() as u8
}

fn normalize(counts: [u32; BIN_COUNT], max_count: u32) -> [f32; BIN_COUNT] {
    let mut normalized = [0.0; BIN_COUNT];
    for (index, count) in counts.into_iter().enumerate() {
        normalized[index] = count as f32 / max_count as f32;
    }
    normalized
}

fn paint_background(ui: &Ui, rect: Rect) {
    ui.painter().rect(
        rect,
        3,
        color::SURFACE_DARK,
        Stroke::new(1.0, color::SURFACE_STRONG.linear_multiply(0.55)),
        StrokeKind::Inside,
    );
}

fn paint_histogram(ui: &Ui, rect: Rect, data: &HistogramData) {
    let painter = ui.painter().with_clip_rect(rect);

    paint_plot_background(&painter, rect);
    paint_grid(&painter, rect);

    paint_filled_series(
        &painter,
        rect,
        data.luma(),
        Color32::from_rgba_unmultiplied(210, 210, 210, 162),
    );
    paint_filled_series(
        &painter,
        rect,
        data.red(),
        Color32::from_rgba_unmultiplied(255, 46, 39, 126),
    );
    paint_filled_series(
        &painter,
        rect,
        data.green(),
        Color32::from_rgba_unmultiplied(58, 222, 74, 116),
    );
    paint_filled_series(
        &painter,
        rect,
        data.blue(),
        Color32::from_rgba_unmultiplied(61, 148, 255, 128),
    );

    paint_series(
        &painter,
        rect,
        data.luma(),
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(235, 235, 235, 150)),
    );
    paint_plot_border(&painter, rect);
}

fn paint_plot_background(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(rect, 1.0, color::SURFACE);
}

fn paint_grid(painter: &egui::Painter, rect: Rect) {
    let stroke = Stroke::new(1.0, color::SURFACE_DARK.linear_multiply(0.58));

    for index in 1..GRID_DIVISIONS {
        let fraction = index as f32 / GRID_DIVISIONS as f32;
        let x = rect.left() + rect.width() * fraction;
        painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], stroke);

        let y = rect.top() + rect.height() * fraction;
        painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], stroke);
    }
}

fn paint_plot_border(painter: &egui::Painter, rect: Rect) {
    painter.rect_stroke(
        rect,
        1.0,
        Stroke::new(1.0, color::BLACK.linear_multiply(0.85)),
        StrokeKind::Inside,
    );
}

fn paint_filled_series(
    painter: &egui::Painter,
    rect: Rect,
    values: &[f32; BIN_COUNT],
    fill: Color32,
) {
    let points = histogram_points(rect, values);
    let mut mesh = Mesh::default();

    for window in points.windows(2) {
        let left_top = window[0];
        let right_top = window[1];
        let left_bottom = pos2(left_top.x, rect.bottom());
        let right_bottom = pos2(right_top.x, rect.bottom());
        let index = mesh.vertices.len() as u32;

        mesh.colored_vertex(left_bottom, fill);
        mesh.colored_vertex(left_top, fill);
        mesh.colored_vertex(right_top, fill);
        mesh.colored_vertex(right_bottom, fill);
        mesh.add_triangle(index, index + 1, index + 2);
        mesh.add_triangle(index, index + 2, index + 3);
    }

    painter.add(egui::Shape::mesh(mesh));
}

fn paint_series(painter: &egui::Painter, rect: Rect, values: &[f32; BIN_COUNT], stroke: Stroke) {
    painter.add(egui::Shape::line(histogram_points(rect, values), stroke));
}

fn histogram_points(rect: Rect, values: &[f32; BIN_COUNT]) -> Vec<Pos2> {
    let bin_width = rect.width() / (BIN_COUNT - 1) as f32;
    let bottom = rect.bottom();
    let height = rect.height();

    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let x = rect.left() + index as f32 * bin_width;
            let y = bottom - value.clamp(0.0, 1.0) * height;
            Pos2::new(x, y)
        })
        .collect()
}

fn paint_loading_indicator(ui: &Ui, rect: Rect) {
    let size = 12.0;
    let spinner_rect = Rect::from_center_size(rect.center(), Vec2::splat(size));
    egui::Spinner::new()
        .size(size)
        .color(color::SURFACE_EMPHASIS)
        .paint_at(ui, spinner_rect);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use image::{DynamicImage, Rgba, RgbaImage};

    #[test]
    fn histogram_widget_renders_with_kittest() {
        let histogram = sample_histogram();

        let mut harness = Harness::new_ui(|ui| {
            ui.set_width(260.0);
            let response = ui.add(Histogram::new(&histogram).height(92.0));

            assert_eq!(response.rect.width(), 260.0);
            assert_eq!(response.rect.height(), 92.0);
        });

        harness.run();
    }

    #[cfg(all(feature = "wgpu", feature = "snapshot"))]
    #[test]
    fn histogram_lightroom_style_snapshot() {
        use egui_kittest::SnapshotResults;

        let histogram = sample_histogram();
        let mut results = SnapshotResults::new();
        let mut harness = Harness::new_ui(|ui| {
            ui.set_width(260.0);
            ui.add(Histogram::new(&histogram).height(92.0));
        });

        harness.fit_contents();
        harness.snapshot("histogram_lightroom_style");
        results.extend_harness(&mut harness);
        results.assert();
    }

    fn sample_histogram() -> HistogramData {
        let image = DynamicImage::ImageRgba8(RgbaImage::from_fn(128, 64, |x, y| {
            let red = ((x as f32 / 127.0).powf(0.8) * 255.0) as u8;
            let green_peak = 1.0 - ((x as f32 - 68.0).abs() / 68.0).clamp(0.0, 1.0);
            let green = (green_peak.powf(1.8) * 255.0) as u8;
            let blue =
                (((127 - x) as f32 / 127.0).powf(1.2) * 180.0 + (y as f32 / 63.0) * 75.0) as u8;

            Rgba([red, green, blue, 255])
        }));

        HistogramData::from_image(&image)
    }
}
