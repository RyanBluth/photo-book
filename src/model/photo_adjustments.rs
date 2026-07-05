use image::{DynamicImage, RgbaImage};
use serde::{Deserialize, Serialize};

const ADJUSTMENT_IDENTITY_EPSILON: f32 = 0.005;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PhotoAdjustments {
    pub light: LightAdjustments,
    pub color: ColorAdjustments,
    pub black_white: BlackWhiteAdjustments,
    pub white_balance: WhiteBalanceAdjustments,
    pub curves: CurvesAdjustments,
    pub levels: LevelsAdjustments,
    pub definition: DefinitionAdjustments,
}

impl PhotoAdjustments {
    pub fn is_identity(&self) -> bool {
        self.canonicalized() == Self::default()
    }

    pub fn cache_key(&self) -> String {
        format!("{:?}", self.canonicalized())
    }

    pub fn canonicalized(&self) -> Self {
        let mut adjustments = self.clone();
        adjustments.snap_to_defaults();
        adjustments
    }

    pub fn gpu_preview_supported(&self) -> bool {
        let adjustments = self.canonicalized();
        adjustments.color.cast == 0.0
    }

    pub fn curve_lut_rgba8(&self) -> [u8; 256 * 4] {
        let mut lut = [0; 256 * 4];

        for index in 0..256 {
            let value = index as f32 / 255.0;
            let rgb = apply_curve_value(value, &self.curves.rgb);
            lut[index * 4] = to_u8(apply_curve_value(rgb, &self.curves.red));
            lut[index * 4 + 1] = to_u8(apply_curve_value(rgb, &self.curves.green));
            lut[index * 4 + 2] = to_u8(apply_curve_value(rgb, &self.curves.blue));
            lut[index * 4 + 3] = 255;
        }

        lut
    }

    pub fn apply_to_image(&self, image: DynamicImage) -> RgbaImage {
        let mut rgba = image.to_rgba8();
        let alpha = alpha_channel(&rgba);
        restore_alpha_channel(&mut rgba, &alpha);
        self.apply_pixel_adjustments(&mut rgba);

        if self.definition.amount > 0.0 {
            let alpha = alpha_channel(&rgba);
            let amount = self.definition.amount.clamp(0.0, 1.0);
            let image = DynamicImage::ImageRgba8(rgba).unsharpen(0.65 + amount * 0.85, 8);
            rgba = image.to_rgba8();
            restore_alpha_channel(&mut rgba, &alpha);
        }

        if self.black_white.grain > 0.0 {
            apply_grain(&mut rgba, self.black_white.grain);
        }

        rgba
    }

    fn apply_pixel_adjustments(&self, image: &mut RgbaImage) {
        for pixel in image.chunks_exact_mut(4) {
            let [red, green, blue] = self.apply_rgb(pixel[0], pixel[1], pixel[2]);
            pixel[0] = red;
            pixel[1] = green;
            pixel[2] = blue;
        }
    }

    fn apply_rgb(&self, red: u8, green: u8, blue: u8) -> [u8; 3] {
        let mut r = f32::from(red) / 255.0;
        let mut g = f32::from(green) / 255.0;
        let mut b = f32::from(blue) / 255.0;

        let exposure = 2.0_f32.powf(self.light.exposure);
        r *= exposure;
        g *= exposure;
        b *= exposure;

        let brightness = self.light.brightness * 0.25;
        r += brightness;
        g += brightness;
        b += brightness;

        let contrast = 1.0 + self.light.contrast * 0.6;
        r = (r - 0.5) * contrast + 0.5;
        g = (g - 0.5) * contrast + 0.5;
        b = (b - 0.5) * contrast + 0.5;

        let luminance = luma(r, g, b);
        let shadow_weight = (1.0 - luminance * 2.0).clamp(0.0, 1.0);
        let highlight_weight = ((luminance - 0.5) * 2.0).clamp(0.0, 1.0);
        let shadow_lift = self.light.shadows * 0.35 * shadow_weight;
        let highlight_pull = self.light.highlights * 0.35 * highlight_weight;
        r += shadow_lift - highlight_pull;
        g += shadow_lift - highlight_pull;
        b += shadow_lift - highlight_pull;

        let black_point = self.light.black_point * 0.2;
        r = (r - black_point) / (1.0 - black_point).max(0.01);
        g = (g - black_point) / (1.0 - black_point).max(0.01);
        b = (b - black_point) / (1.0 - black_point).max(0.01);

        let brilliance = self.light.brilliance * 0.25;
        let luminance = luma(r, g, b);
        r += (r - luminance) * brilliance + brilliance * 0.35;
        g += (g - luminance) * brilliance + brilliance * 0.35;
        b += (b - luminance) * brilliance + brilliance * 0.35;

        let temperature = self.white_balance.temperature * 0.12;
        let tint = self.white_balance.tint * 0.10;
        r += temperature;
        b -= temperature;
        g += tint;
        r -= tint * 0.5;
        b -= tint * 0.5;

        let saturation = 1.0 + self.color.saturation * 0.8;
        let vibrance = self.color.vibrance * (1.0 - saturation_distance(r, g, b)) * 0.8;
        let sat_factor = saturation + vibrance;
        let luminance = luma(r, g, b);
        r = luminance + (r - luminance) * sat_factor;
        g = luminance + (g - luminance) * sat_factor;
        b = luminance + (b - luminance) * sat_factor;

        let cast = self.color.cast * 0.08;
        r += cast;
        g += cast * 0.3;
        b -= cast;

        let black_white_amount = self.black_white.intensity.clamp(0.0, 1.0);
        if black_white_amount > 0.0 {
            let tone = self.black_white.tone * 0.12;
            let neutral = self.black_white.neutrals * 0.12;
            let bw = luma(r, g, b) + neutral;
            r = lerp(r, bw + tone, black_white_amount);
            g = lerp(g, bw, black_white_amount);
            b = lerp(b, bw - tone, black_white_amount);
        }

        r = apply_curve_value(r, &self.curves.rgb);
        g = apply_curve_value(g, &self.curves.rgb);
        b = apply_curve_value(b, &self.curves.rgb);
        r = apply_curve_value(r, &self.curves.red);
        g = apply_curve_value(g, &self.curves.green);
        b = apply_curve_value(b, &self.curves.blue);

        let before_luminance = luma(r, g, b);
        let after_luminance = apply_levels_value(before_luminance, &self.levels.luminance);
        if before_luminance > f32::EPSILON {
            let scale = after_luminance / before_luminance;
            r *= scale;
            g *= scale;
            b *= scale;
        } else {
            r += after_luminance;
            g += after_luminance;
            b += after_luminance;
        }

        r = apply_levels_value(r, &self.levels.rgb);
        g = apply_levels_value(g, &self.levels.rgb);
        b = apply_levels_value(b, &self.levels.rgb);
        r = apply_levels_value(r, &self.levels.red);
        g = apply_levels_value(g, &self.levels.green);
        b = apply_levels_value(b, &self.levels.blue);

        [to_u8(r), to_u8(g), to_u8(b)]
    }

    fn snap_to_defaults(&mut self) {
        let default = Self::default();

        snap_to_default(&mut self.light.exposure, default.light.exposure);
        snap_to_default(&mut self.light.brilliance, default.light.brilliance);
        snap_to_default(&mut self.light.highlights, default.light.highlights);
        snap_to_default(&mut self.light.shadows, default.light.shadows);
        snap_to_default(&mut self.light.contrast, default.light.contrast);
        snap_to_default(&mut self.light.brightness, default.light.brightness);
        snap_to_default(&mut self.light.black_point, default.light.black_point);

        snap_to_default(&mut self.color.saturation, default.color.saturation);
        snap_to_default(&mut self.color.vibrance, default.color.vibrance);
        snap_to_default(&mut self.color.cast, default.color.cast);

        snap_to_default(
            &mut self.black_white.intensity,
            default.black_white.intensity,
        );
        snap_to_default(&mut self.black_white.neutrals, default.black_white.neutrals);
        snap_to_default(&mut self.black_white.tone, default.black_white.tone);
        snap_to_default(&mut self.black_white.grain, default.black_white.grain);

        snap_to_default(
            &mut self.white_balance.temperature,
            default.white_balance.temperature,
        );
        snap_to_default(&mut self.white_balance.tint, default.white_balance.tint);

        snap_level_values(&mut self.levels.luminance, &default.levels.luminance);
        snap_level_values(&mut self.levels.rgb, &default.levels.rgb);
        snap_level_values(&mut self.levels.red, &default.levels.red);
        snap_level_values(&mut self.levels.green, &default.levels.green);
        snap_level_values(&mut self.levels.blue, &default.levels.blue);

        snap_to_default(&mut self.definition.amount, default.definition.amount);
    }
}

fn snap_to_default(value: &mut f32, default: f32) {
    if (*value - default).abs() < ADJUSTMENT_IDENTITY_EPSILON {
        *value = default;
    }
}

fn snap_level_values(values: &mut LevelValues, default: &LevelValues) {
    snap_to_default(&mut values.black, default.black);
    snap_to_default(&mut values.mid, default.mid);
    snap_to_default(&mut values.white, default.white);
}

fn alpha_channel(image: &RgbaImage) -> Vec<u8> {
    image.pixels().map(|pixel| pixel[3]).collect()
}

fn restore_alpha_channel(image: &mut RgbaImage, alpha: &[u8]) {
    for (pixel, alpha) in image.pixels_mut().zip(alpha.iter().copied()) {
        pixel[3] = alpha;
    }
}

fn apply_grain(image: &mut RgbaImage, amount: f32) {
    let amount = amount.clamp(0.0, 1.0);
    if amount <= f32::EPSILON {
        return;
    }

    let strength = amount * 28.0;
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let noise = hash_noise(x, y) * 2.0 - 1.0;
        let delta = noise * strength;
        pixel[0] = add_delta(pixel[0], delta);
        pixel[1] = add_delta(pixel[1], delta);
        pixel[2] = add_delta(pixel[2], delta);
    }
}

fn hash_noise(x: u32, y: u32) -> f32 {
    let mut value = x
        .wrapping_mul(0x045d_9f3b)
        .wrapping_add(y.wrapping_mul(0x119d_e1f3));
    value ^= value >> 16;
    value = value.wrapping_mul(0x045d_9f3b);
    value ^= value >> 16;
    value as f32 / u32::MAX as f32
}

fn add_delta(value: u8, delta: f32) -> u8 {
    (f32::from(value) + delta).clamp(0.0, 255.0).round() as u8
}

fn luma(red: f32, green: f32, blue: f32) -> f32 {
    red * 0.2126 + green * 0.7152 + blue * 0.0722
}

fn saturation_distance(red: f32, green: f32, blue: f32) -> f32 {
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    (max - min).clamp(0.0, 1.0)
}

fn lerp(left: f32, right: f32, amount: f32) -> f32 {
    left + (right - left) * amount
}

fn to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn apply_curve_value(value: f32, curve: &Curve) -> f32 {
    let value = value.clamp(0.0, 1.0);
    let mut points = curve.points.clone();
    points.sort_by(|left, right| {
        left.x
            .partial_cmp(&right.x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for window in points.windows(2) {
        let left = &window[0];
        let right = &window[1];
        if value >= left.x && value <= right.x {
            let span = (right.x - left.x).max(0.001);
            let amount = (value - left.x) / span;
            return lerp(left.y, right.y, amount).clamp(0.0, 1.0);
        }
    }

    value
}

fn apply_levels_value(value: f32, levels: &LevelValues) -> f32 {
    let black = levels.black.clamp(0.0, 0.98);
    let white = levels.white.clamp(black + 0.01, 1.0);
    let mid = levels.mid.clamp(0.05, 0.95);
    let normalized = ((value - black) / (white - black)).clamp(0.0, 1.0);
    normalized.powf((1.0 - mid).max(0.05) / mid.max(0.05))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LightAdjustments {
    pub exposure: f32,
    pub brilliance: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub contrast: f32,
    pub brightness: f32,
    pub black_point: f32,
}

impl Default for LightAdjustments {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            brilliance: 0.0,
            highlights: 0.0,
            shadows: 0.0,
            contrast: 0.0,
            brightness: 0.0,
            black_point: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorAdjustments {
    pub saturation: f32,
    pub vibrance: f32,
    pub cast: f32,
}

impl Default for ColorAdjustments {
    fn default() -> Self {
        Self {
            saturation: 0.0,
            vibrance: 0.0,
            cast: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlackWhiteAdjustments {
    pub intensity: f32,
    pub neutrals: f32,
    pub tone: f32,
    pub grain: f32,
}

impl Default for BlackWhiteAdjustments {
    fn default() -> Self {
        Self {
            intensity: 0.0,
            neutrals: 0.0,
            tone: 0.0,
            grain: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhiteBalanceAdjustments {
    pub temperature: f32,
    pub tint: f32,
}

impl Default for WhiteBalanceAdjustments {
    fn default() -> Self {
        Self {
            temperature: 0.0,
            tint: 0.0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CurvesAdjustments {
    pub rgb: Curve,
    pub red: Curve,
    pub green: Curve,
    pub blue: Curve,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Curve {
    pub points: Vec<CurvePoint>,
}

impl Default for Curve {
    fn default() -> Self {
        Self {
            points: vec![CurvePoint { x: 0.0, y: 0.0 }, CurvePoint { x: 1.0, y: 1.0 }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurvePoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelValues {
    pub black: f32,
    pub mid: f32,
    pub white: f32,
}

impl Default for LevelValues {
    fn default() -> Self {
        Self {
            black: 0.0,
            mid: 0.5,
            white: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LevelsAdjustments {
    pub luminance: LevelValues,
    pub rgb: LevelValues,
    pub red: LevelValues,
    pub green: LevelValues,
    pub blue: LevelValues,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DefinitionAdjustments {
    pub amount: f32,
}

impl Default for DefinitionAdjustments {
    fn default() -> Self {
        Self { amount: 0.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgba, RgbaImage};

    #[test]
    fn red_levels_affect_red_channel_only() {
        let mut adjustments = PhotoAdjustments::default();
        adjustments.levels.red.black = 0.5;

        let image =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([128, 128, 128, 255])));
        let adjusted = adjustments.apply_to_image(image);
        let pixel = adjusted.get_pixel(0, 0);

        assert!(pixel[0] < pixel[1]);
        assert_eq!(pixel[1], pixel[2]);
    }

    #[test]
    fn default_adjustments_preserve_pixels() {
        let image = DynamicImage::ImageRgba8(RgbaImage::from_fn(4, 4, |x, y| {
            Rgba([
                (x * 47 + y * 11) as u8,
                (x * 23 + y * 41) as u8,
                (x * 17 + y * 53) as u8,
                (64 + x * 12 + y * 7) as u8,
            ])
        }));
        let source = image.to_rgba8();
        let adjusted = PhotoAdjustments::default().apply_to_image(image);

        assert_eq!(adjusted, source);
    }

    #[test]
    fn default_curve_lut_is_identity() {
        let lut = PhotoAdjustments::default().curve_lut_rgba8();

        for value in 0..=255 {
            assert_eq!(lut[value * 4], value as u8);
            assert_eq!(lut[value * 4 + 1], value as u8);
            assert_eq!(lut[value * 4 + 2], value as u8);
            assert_eq!(lut[value * 4 + 3], 255);
        }
    }

    #[test]
    fn grain_changes_rgb_and_preserves_alpha() {
        let mut adjustments = PhotoAdjustments::default();
        adjustments.black_white.grain = 1.0;

        let image =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([128, 128, 128, 77])));
        let adjusted = adjustments.apply_to_image(image);
        let pixel = adjusted.get_pixel(0, 0);

        assert_ne!([pixel[0], pixel[1], pixel[2]], [128, 128, 128]);
        assert_eq!(pixel[3], 77);
    }

    #[test]
    fn grain_is_gpu_preview_supported() {
        let mut adjustments = PhotoAdjustments::default();
        adjustments.black_white.grain = 1.0;

        assert!(adjustments.gpu_preview_supported());
    }

    #[test]
    fn sharpening_is_gpu_preview_supported() {
        let mut adjustments = PhotoAdjustments::default();
        adjustments.definition.amount = 1.0;

        assert!(adjustments.gpu_preview_supported());
    }

    #[test]
    fn tiny_light_adjustments_have_small_pixel_deltas() {
        let source = DynamicImage::ImageRgba8(RgbaImage::from_fn(5, 1, |x, _| {
            let value = [24, 64, 128, 192, 232][x as usize];
            Rgba([value, value.saturating_add(8), value.saturating_sub(8), 255])
        }));
        let baseline = PhotoAdjustments::default().apply_to_image(source.clone());

        let light_adjustments = [
            |light: &mut LightAdjustments| light.exposure = 0.01,
            |light: &mut LightAdjustments| light.brilliance = 0.01,
            |light: &mut LightAdjustments| light.highlights = 0.01,
            |light: &mut LightAdjustments| light.shadows = 0.01,
            |light: &mut LightAdjustments| light.contrast = 0.01,
            |light: &mut LightAdjustments| light.brightness = 0.01,
            |light: &mut LightAdjustments| light.black_point = 0.01,
        ];

        for adjust_light in light_adjustments {
            let mut adjustments = PhotoAdjustments::default();
            adjust_light(&mut adjustments.light);
            let adjusted = adjustments.apply_to_image(source.clone());

            for (baseline_pixel, adjusted_pixel) in baseline.pixels().zip(adjusted.pixels()) {
                for channel in 0..3 {
                    let delta = baseline_pixel[channel].abs_diff(adjusted_pixel[channel]);
                    assert!(
                        delta <= 3,
                        "expected tiny light adjustment delta <= 3, got {delta}"
                    );
                }
                assert_eq!(baseline_pixel[3], adjusted_pixel[3]);
            }
        }
    }

    #[test]
    fn visually_zero_adjustments_are_identity() {
        let mut adjustments = PhotoAdjustments::default();
        adjustments.light.exposure = 0.004;
        adjustments.light.brightness = -0.004;
        adjustments.color.saturation = 0.004;
        adjustments.black_white.grain = 0.004;
        adjustments.definition.amount = 0.004;

        assert!(adjustments.is_identity());
        assert_eq!(adjustments.canonicalized(), PhotoAdjustments::default());
        assert_eq!(
            adjustments.cache_key(),
            PhotoAdjustments::default().cache_key()
        );
    }

    #[test]
    fn visible_adjustments_are_not_identity() {
        let mut adjustments = PhotoAdjustments::default();
        adjustments.light.exposure = 0.006;

        assert!(!adjustments.is_identity());
    }
}
