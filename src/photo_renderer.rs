use eframe::egui::{self, Rect, load::SizedTexture};
use egui::{Mesh, Pos2, Shape, emath::Rot2};

use crate::{
    dep, dep_mut,
    gpu_photo_adjustment::{
        GpuPaintResult, GpuPhotoAdjustmentRenderer, GpuPhotoPaintRequest, GpuPhotoSource,
    },
    model::photo_adjustments::PhotoAdjustments,
    photo::Photo,
    photo_manager::{PhotoManager, PhotoTextureOptions},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhotoRenderSource {
    FullResolution,
    Thumbnail,
}

/// Controls how [`PhotoRenderer`] paints a photo.
#[derive(Clone, Copy, Debug)]
pub struct PhotoRenderOptions<'a> {
    source: PhotoRenderSource,
    use_gpu: bool,
    clip_rect: Option<Rect>,
    source_uv: Rect,
    rotation_radians: f32,
    render_key: Option<&'a str>,
}

impl Default for PhotoRenderOptions<'_> {
    fn default() -> Self {
        Self {
            source: PhotoRenderSource::FullResolution,
            use_gpu: true,
            clip_rect: None,
            source_uv: Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            rotation_radians: 0.0,
            render_key: None,
        }
    }
}

impl<'a> PhotoRenderOptions<'a> {
    pub fn thumbnail(mut self) -> Self {
        self.source = PhotoRenderSource::Thumbnail;
        self
    }

    pub fn gpu(mut self, enabled: bool) -> Self {
        self.use_gpu = enabled;
        self
    }

    pub fn with_clip_rect(mut self, clip_rect: Rect) -> Self {
        self.clip_rect = Some(clip_rect);
        self
    }

    pub fn with_crop(mut self, source_uv: Rect) -> Self {
        self.source_uv = source_uv;
        self
    }

    pub fn with_rotation(mut self, rotation_radians: f32) -> Self {
        self.rotation_radians = rotation_radians;
        self
    }

    pub fn with_render_key(mut self, render_key: &'a str) -> Self {
        self.render_key = Some(render_key);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhotoRenderStatus {
    Ready,
    Placeholder,
    Pending,
    NotVisible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhotoRenderRoute {
    Complete(PhotoRenderStatus),
    Placeholder,
    Cpu,
}

/// Paints photos through the GPU adjustment renderer when possible and falls back to textures.
pub struct PhotoRenderer;

impl PhotoRenderer {
    pub fn paint(
        ui: &mut egui::Ui,
        photo: &Photo,
        adjustments: &PhotoAdjustments,
        rect: Rect,
        options: PhotoRenderOptions<'_>,
    ) -> anyhow::Result<PhotoRenderStatus> {
        let clip_rect = options.clip_rect.unwrap_or_else(|| ui.clip_rect());
        let gpu_result = (options.use_gpu && !adjustments.is_identity()).then(|| {
            dep!(GpuPhotoAdjustmentRenderer, |renderer| {
                renderer.paint(
                    ui,
                    GpuPhotoPaintRequest {
                        photo,
                        source: match options.source {
                            PhotoRenderSource::FullResolution => GpuPhotoSource::FullResolution,
                            PhotoRenderSource::Thumbnail => GpuPhotoSource::Thumbnail,
                        },
                        clip_rect,
                        rect,
                        source_uv: options.source_uv,
                        rotation_radians: options.rotation_radians,
                        adjustments,
                        render_key: options.render_key,
                    },
                )
            })
        });

        match Self::route(gpu_result) {
            PhotoRenderRoute::Complete(status) => return Ok(status),
            PhotoRenderRoute::Placeholder => {
                return Self::paint_placeholder(ui, photo, rect, clip_rect, options);
            }
            PhotoRenderRoute::Cpu => {}
        }

        let texture_result = dep_mut!(PhotoManager, |photo_manager| {
            let texture_options = match options.source {
                PhotoRenderSource::FullResolution => PhotoTextureOptions::full_resolution()
                    .with_adjustments(adjustments)
                    .with_thumbnail_fallback(),
                PhotoRenderSource::Thumbnail => {
                    PhotoTextureOptions::thumbnail().with_adjustments(adjustments)
                }
            };
            photo_manager.texture_for(photo, ui.ctx(), texture_options)
        });

        match texture_result {
            Ok(Some(texture)) => {
                Self::paint_texture(ui, texture, rect, clip_rect, options);
                Ok(PhotoRenderStatus::Ready)
            }
            Ok(None) => Self::paint_placeholder(ui, photo, rect, clip_rect, options),
            Err(error) => match Self::paint_placeholder(ui, photo, rect, clip_rect, options) {
                Ok(PhotoRenderStatus::Placeholder) => Ok(PhotoRenderStatus::Placeholder),
                _ => Err(error),
            },
        }
    }

    fn paint_placeholder(
        ui: &mut egui::Ui,
        photo: &Photo,
        rect: Rect,
        clip_rect: Rect,
        options: PhotoRenderOptions<'_>,
    ) -> anyhow::Result<PhotoRenderStatus> {
        let texture = dep_mut!(PhotoManager, |photo_manager| {
            photo_manager.texture_for(
                photo,
                ui.ctx(),
                PhotoTextureOptions::thumbnail().without_adjustments(),
            )
        })?;

        if let Some(texture) = texture {
            Self::paint_texture(ui, texture, rect, clip_rect, options);
            Ok(PhotoRenderStatus::Placeholder)
        } else {
            Ok(PhotoRenderStatus::Pending)
        }
    }

    fn route(gpu_result: Option<GpuPaintResult>) -> PhotoRenderRoute {
        match gpu_result {
            Some(GpuPaintResult::Ready) => PhotoRenderRoute::Complete(PhotoRenderStatus::Ready),
            Some(GpuPaintResult::NotVisible) => {
                PhotoRenderRoute::Complete(PhotoRenderStatus::NotVisible)
            }
            Some(GpuPaintResult::Pending) => PhotoRenderRoute::Placeholder,
            Some(GpuPaintResult::Unsupported) | None => PhotoRenderRoute::Cpu,
        }
    }

    fn paint_texture(
        ui: &egui::Ui,
        texture: SizedTexture,
        rect: Rect,
        clip_rect: Rect,
        options: PhotoRenderOptions<'_>,
    ) {
        let mut mesh = Mesh::with_texture(texture.id);
        mesh.add_rect_with_uv(rect, options.source_uv, egui::Color32::WHITE);
        mesh.rotate(Rot2::from_angle(options.rotation_radians), rect.center());
        ui.painter()
            .with_clip_rect(clip_rect)
            .add(Shape::mesh(mesh));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_options_default_to_gpu_full_resolution() {
        let options = PhotoRenderOptions::default();

        assert!(options.use_gpu);
        assert_eq!(options.source, PhotoRenderSource::FullResolution);
        assert_eq!(
            options.source_uv,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))
        );
        assert_eq!(options.rotation_radians, 0.0);
        assert!(options.clip_rect.is_none());
        assert!(options.render_key.is_none());
    }

    #[test]
    fn render_options_build_thumbnail_transform() {
        let crop = Rect::from_min_max(Pos2::new(0.1, 0.2), Pos2::new(0.8, 0.9));
        let clip = Rect::from_min_max(Pos2::new(10.0, 20.0), Pos2::new(30.0, 40.0));
        let options = PhotoRenderOptions::default()
            .thumbnail()
            .gpu(false)
            .with_clip_rect(clip)
            .with_crop(crop)
            .with_rotation(0.5)
            .with_render_key("photo");

        assert!(!options.use_gpu);
        assert_eq!(options.source, PhotoRenderSource::Thumbnail);
        assert_eq!(options.clip_rect, Some(clip));
        assert_eq!(options.source_uv, crop);
        assert_eq!(options.rotation_radians, 0.5);
        assert_eq!(options.render_key, Some("photo"));
    }

    #[test]
    fn gpu_result_selects_the_expected_fallback_route() {
        assert_eq!(
            PhotoRenderer::route(Some(GpuPaintResult::Ready)),
            PhotoRenderRoute::Complete(PhotoRenderStatus::Ready)
        );
        assert_eq!(
            PhotoRenderer::route(Some(GpuPaintResult::NotVisible)),
            PhotoRenderRoute::Complete(PhotoRenderStatus::NotVisible)
        );
        assert_eq!(
            PhotoRenderer::route(Some(GpuPaintResult::Pending)),
            PhotoRenderRoute::Placeholder
        );
        assert_eq!(
            PhotoRenderer::route(Some(GpuPaintResult::Unsupported)),
            PhotoRenderRoute::Cpu
        );
        assert_eq!(PhotoRenderer::route(None), PhotoRenderRoute::Cpu);
    }
}
