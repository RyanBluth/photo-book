use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use eframe::{
    egui::{self, Rect, Shape},
    egui_wgpu::{self, wgpu, wgpu::util::DeviceExt},
};
use log::error;
use parking_lot::Mutex;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::task::spawn_blocking;

use crate::{
    image_utils::{decode_oriented_image, path_version_key},
    model::photo_adjustments::PhotoAdjustments,
    photo::Photo,
    utils::RectExt,
};

const CURVE_LUT_SIZE: u32 = 256;
const CURVE_LUT_BYTES: usize = CURVE_LUT_SIZE as usize * 4;
const GPU_SOURCE_CACHE_CAPACITY: usize = 12;
const GPU_RENDER_CACHE_CAPACITY: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct AdjustmentUniform {
    exposure: f32,
    brightness: f32,
    contrast: f32,
    saturation: f32,
    vibrance: f32,
    temperature: f32,
    tint: f32,
    black_white_intensity: f32,
    black_white_tone: f32,
    black_white_neutrals: f32,
    grain: f32,
    black_point: f32,
    brilliance: f32,
    highlights: f32,
    shadows: f32,
    level_luminance_black: f32,
    level_luminance_mid: f32,
    level_luminance_white: f32,
    level_rgb_black: f32,
    level_rgb_mid: f32,
    level_rgb_white: f32,
    level_red_black: f32,
    level_red_mid: f32,
    level_red_white: f32,
    level_green_black: f32,
    level_green_mid: f32,
    level_green_white: f32,
    level_blue_black: f32,
    level_blue_mid: f32,
    level_blue_white: f32,
    definition_amount: f32,
    rotation_radians: f32,
    uv_min_x: f32,
    uv_min_y: f32,
    uv_span_x: f32,
    uv_span_y: f32,
    source_uv_min_x: f32,
    source_uv_min_y: f32,
    source_uv_span_x: f32,
    source_uv_span_y: f32,
}

impl AdjustmentUniform {
    fn new(
        adjustments: &PhotoAdjustments,
        rotation_radians: f32,
        callback_rect: Rect,
        image_rect: Rect,
        source_uv: Rect,
    ) -> Self {
        let image_size = image_rect.size();
        let uv_min = if image_size.x > 0.0 && image_size.y > 0.0 {
            (callback_rect.min - image_rect.min) / image_size
        } else {
            egui::Vec2::ZERO
        };
        let uv_max = if image_size.x > 0.0 && image_size.y > 0.0 {
            (callback_rect.max - image_rect.min) / image_size
        } else {
            egui::Vec2::ZERO
        };

        Self {
            exposure: adjustments.light.exposure,
            brightness: adjustments.light.brightness,
            contrast: adjustments.light.contrast,
            saturation: adjustments.color.saturation,
            vibrance: adjustments.color.vibrance,
            temperature: adjustments.white_balance.temperature,
            tint: adjustments.white_balance.tint,
            black_white_intensity: adjustments.black_white.intensity,
            black_white_tone: adjustments.black_white.tone,
            black_white_neutrals: adjustments.black_white.neutrals,
            grain: adjustments.black_white.grain,
            black_point: adjustments.light.black_point,
            brilliance: adjustments.light.brilliance,
            highlights: adjustments.light.highlights,
            shadows: adjustments.light.shadows,
            level_luminance_black: adjustments.levels.luminance.black,
            level_luminance_mid: adjustments.levels.luminance.mid,
            level_luminance_white: adjustments.levels.luminance.white,
            level_rgb_black: adjustments.levels.rgb.black,
            level_rgb_mid: adjustments.levels.rgb.mid,
            level_rgb_white: adjustments.levels.rgb.white,
            level_red_black: adjustments.levels.red.black,
            level_red_mid: adjustments.levels.red.mid,
            level_red_white: adjustments.levels.red.white,
            level_green_black: adjustments.levels.green.black,
            level_green_mid: adjustments.levels.green.mid,
            level_green_white: adjustments.levels.green.white,
            level_blue_black: adjustments.levels.blue.black,
            level_blue_mid: adjustments.levels.blue.mid,
            level_blue_white: adjustments.levels.blue.white,
            definition_amount: adjustments.definition.amount,
            rotation_radians: -rotation_radians,
            uv_min_x: uv_min.x,
            uv_min_y: uv_min.y,
            uv_span_x: uv_max.x - uv_min.x,
            uv_span_y: uv_max.y - uv_min.y,
            source_uv_min_x: source_uv.min.x,
            source_uv_min_y: source_uv.min.y,
            source_uv_span_x: source_uv.width(),
            source_uv_span_y: source_uv.height(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuPaintResult {
    Unsupported,
    NotVisible,
    Pending,
    Ready,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuPhotoSource {
    FullResolution,
    Thumbnail,
}

pub struct GpuPhotoPaintRequest<'a> {
    pub photo: &'a Photo,
    pub source: GpuPhotoSource,
    pub clip_rect: Rect,
    pub rect: Rect,
    pub source_uv: Rect,
    pub rotation_radians: f32,
    pub adjustments: &'a PhotoAdjustments,
    pub render_key: Option<&'a str>,
}

#[derive(Debug)]
pub struct GpuPhotoAdjustmentRenderer {
    enabled: bool,
    render_status: Arc<Mutex<HashMap<String, GpuSourceStatus>>>,
}

impl Default for GpuPhotoAdjustmentRenderer {
    fn default() -> Self {
        Self {
            enabled: false,
            render_status: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GpuSourceStatus {
    Pending,
    Ready,
    Failed,
}

fn set_render_status(
    statuses: &Arc<Mutex<HashMap<String, GpuSourceStatus>>>,
    render_key: &str,
    status: GpuSourceStatus,
) -> bool {
    statuses.lock().insert(render_key.to_owned(), status) != Some(status)
}

fn publish_terminal_render_status(
    statuses: &Arc<Mutex<HashMap<String, GpuSourceStatus>>>,
    render_key: &str,
    status: GpuSourceStatus,
    repaint_ctx: &egui::Context,
) {
    if set_render_status(statuses, render_key, status) {
        repaint_ctx.request_repaint();
    }
}

impl GpuPhotoAdjustmentRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn initialize(&mut self, cc: &eframe::CreationContext<'_>) {
        let Some(render_state) = cc.wgpu_render_state.as_ref() else {
            self.enabled = false;
            return;
        };

        let device = &render_state.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("photo adjustment shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_photo_adjustment.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("photo adjustment bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("photo adjustment pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("photo adjustment pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: render_state.target_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("photo adjustment sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        render_state
            .renderer
            .write()
            .callback_resources
            .insert(PhotoAdjustmentRenderResources {
                pipeline,
                bind_group_layout,
                sampler,
                sources: HashMap::new(),
                renders: HashMap::new(),
                pending_sources: HashMap::new(),
                render_status: self.render_status.clone(),
                access_counter: 0,
            });

        self.enabled = true;
    }

    pub fn paint(&self, ui: &mut egui::Ui, request: GpuPhotoPaintRequest<'_>) -> GpuPaintResult {
        if !self.enabled || !request.adjustments.gpu_preview_supported() {
            return GpuPaintResult::Unsupported;
        }

        let (path, source_key, max_texture_side) = match request.source {
            GpuPhotoSource::FullResolution => (
                request.photo.path.clone(),
                request.photo.uri(),
                ui.ctx().input(|input| input.max_texture_side as u32),
            ),
            GpuPhotoSource::Thumbnail => {
                let Ok(path) = request.photo.thumbnail_path() else {
                    return GpuPaintResult::Unsupported;
                };
                (path, request.photo.thumbnail_uri(), 0)
            }
        };
        if !path.exists() {
            return GpuPaintResult::Unsupported;
        }

        self.paint_source(ui, &path, source_key, max_texture_side, request)
    }

    fn paint_source(
        &self,
        ui: &mut egui::Ui,
        path: &Path,
        source_key: String,
        max_texture_side: u32,
        request: GpuPhotoPaintRequest<'_>,
    ) -> GpuPaintResult {
        let paint_bounds = request
            .rect
            .rotate_bb_around_center(request.rotation_radians);
        let callback_rect = request.clip_rect.intersect(paint_bounds);
        if !callback_rect.is_positive() {
            return GpuPaintResult::NotVisible;
        }

        let source_key = gpu_source_cache_key(&source_key, path, max_texture_side);
        let render_key = match request.render_key {
            Some(render_key) => format!("{source_key}:render:{render_key}"),
            None => source_key.clone(),
        };
        let status = self.render_status.lock().get(&render_key).copied();
        if status == Some(GpuSourceStatus::Failed) {
            return GpuPaintResult::Unsupported;
        }

        if status.is_none() {
            self.render_status
                .lock()
                .insert(render_key.clone(), GpuSourceStatus::Pending);
        }

        ui.painter()
            .add(Shape::Callback(egui_wgpu::Callback::new_paint_callback(
                callback_rect,
                PhotoAdjustmentCallback {
                    path: path.to_path_buf(),
                    source_key,
                    render_key,
                    max_texture_side,
                    repaint_ctx: ui.ctx().clone(),
                    uniform: AdjustmentUniform::new(
                        request.adjustments,
                        request.rotation_radians,
                        callback_rect,
                        request.rect,
                        request.source_uv,
                    ),
                    curve_lut: request.adjustments.curve_lut_rgba8(),
                },
            )));

        if status == Some(GpuSourceStatus::Ready) {
            GpuPaintResult::Ready
        } else {
            GpuPaintResult::Pending
        }
    }
}

struct PhotoAdjustmentCallback {
    path: PathBuf,
    source_key: String,
    render_key: String,
    max_texture_side: u32,
    repaint_ctx: egui::Context,
    uniform: AdjustmentUniform,
    curve_lut: [u8; CURVE_LUT_BYTES],
}

impl egui_wgpu::CallbackTrait for PhotoAdjustmentCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(resources) = resources.get_mut::<PhotoAdjustmentRenderResources>() else {
            return Vec::new();
        };

        resources.prepare(
            device,
            queue,
            GpuSourceRequest {
                cache_key: &self.source_key,
                path: &self.path,
                max_texture_side: self.max_texture_side,
                repaint_ctx: &self.repaint_ctx,
            },
            &self.render_key,
            self.uniform,
            &self.curve_lut,
        );
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(resources) = resources.get::<PhotoAdjustmentRenderResources>() else {
            return;
        };

        resources.paint(render_pass, &self.render_key);
    }
}

struct PhotoAdjustmentRenderResources {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    sources: HashMap<String, GpuSourceTexture>,
    renders: HashMap<String, GpuAdjustedPhoto>,
    pending_sources:
        HashMap<String, oneshot::Receiver<std::result::Result<DecodedGpuSource, String>>>,
    render_status: Arc<Mutex<HashMap<String, GpuSourceStatus>>>,
    access_counter: u64,
}

struct GpuSourceRequest<'a> {
    cache_key: &'a str,
    path: &'a Path,
    max_texture_side: u32,
    repaint_ctx: &'a egui::Context,
}

struct GpuRenderUpdate<'a> {
    source_key: &'a str,
    render_key: &'a str,
    uniform: AdjustmentUniform,
    curve_lut: &'a [u8; CURVE_LUT_BYTES],
    access: u64,
}

impl PhotoAdjustmentRenderResources {
    fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: GpuSourceRequest<'_>,
        render_key: &str,
        uniform: AdjustmentUniform,
        curve_lut: &[u8; CURVE_LUT_BYTES],
    ) {
        self.access_counter = self.access_counter.saturating_add(1);
        let access = self.access_counter;

        if self.sources.contains_key(source.cache_key) {
            self.update_render(
                device,
                queue,
                GpuRenderUpdate {
                    source_key: source.cache_key,
                    render_key,
                    uniform,
                    curve_lut,
                    access,
                },
            );
            self.set_terminal_render_status(render_key, GpuSourceStatus::Ready, source.repaint_ctx);
            return;
        }

        let Some(decoded) = self.poll_or_start_decode(&source) else {
            self.set_render_status(render_key, GpuSourceStatus::Pending);
            return;
        };

        let decoded = match decoded {
            Ok(decoded) => decoded,
            Err(error) => {
                error!("Failed to prepare GPU photo adjustment source: {error}");
                self.set_terminal_render_status(
                    render_key,
                    GpuSourceStatus::Failed,
                    source.repaint_ctx,
                );
                return;
            }
        };

        let size = decoded.size();
        if size.width == 0 || size.height == 0 {
            self.set_terminal_render_status(
                render_key,
                GpuSourceStatus::Failed,
                source.repaint_ctx,
            );
            return;
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("photo adjustment source texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &decoded.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * size.width),
                rows_per_image: Some(size.height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.sources.insert(
            source.cache_key.to_owned(),
            GpuSourceTexture {
                _texture: texture,
                view,
                last_access: access,
            },
        );
        self.update_render(
            device,
            queue,
            GpuRenderUpdate {
                source_key: source.cache_key,
                render_key,
                uniform,
                curve_lut,
                access,
            },
        );
        self.evict_old_sources(source.cache_key);
        self.set_terminal_render_status(render_key, GpuSourceStatus::Ready, source.repaint_ctx);
    }

    fn update_render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        update: GpuRenderUpdate<'_>,
    ) {
        if let Some(source_texture) = self.sources.get_mut(update.source_key) {
            source_texture.last_access = update.access;
        }

        let source_changed = self
            .renders
            .get(update.render_key)
            .is_some_and(|photo| photo.source_key != update.source_key);
        if source_changed {
            self.renders.remove(update.render_key);
        }

        if let Some(photo) = self.renders.get_mut(update.render_key) {
            photo.last_access = update.access;
            if photo.uniform != update.uniform {
                queue.write_buffer(
                    &photo.uniform_buffer,
                    0,
                    bytemuck::bytes_of(&update.uniform),
                );
                photo.uniform = update.uniform;
            }
            if &photo.curve_lut != update.curve_lut {
                write_curve_lut(queue, &photo.curve_texture, update.curve_lut);
                photo.curve_lut = *update.curve_lut;
            }
            return;
        }

        let Some(source_texture) = self.sources.get(update.source_key) else {
            return;
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("photo adjustment uniform buffer"),
            contents: bytemuck::bytes_of(&update.uniform),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        });
        let curve_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("photo adjustment curve lut"),
            size: wgpu::Extent3d {
                width: CURVE_LUT_SIZE,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        write_curve_lut(queue, &curve_texture, update.curve_lut);
        let curve_view = curve_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("photo adjustment bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&source_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&curve_view),
                },
            ],
        });

        self.renders.insert(
            update.render_key.to_owned(),
            GpuAdjustedPhoto {
                source_key: update.source_key.to_owned(),
                curve_texture,
                _curve_view: curve_view,
                bind_group,
                uniform_buffer,
                uniform: update.uniform,
                curve_lut: *update.curve_lut,
                last_access: update.access,
            },
        );
        self.evict_old_renders(update.render_key);
    }

    fn poll_or_start_decode(
        &mut self,
        source: &GpuSourceRequest<'_>,
    ) -> Option<std::result::Result<DecodedGpuSource, String>> {
        if let Some(result) = self.poll_source_decode(source.cache_key) {
            return Some(result);
        }

        if !self.pending_sources.contains_key(source.cache_key) {
            let (sender, receiver) = oneshot::channel();
            spawn_gpu_source_decode(
                source.path.to_path_buf(),
                source.max_texture_side,
                sender,
                source.repaint_ctx.clone(),
            );
            self.pending_sources
                .insert(source.cache_key.to_owned(), receiver);
        }

        None
    }

    fn poll_source_decode(
        &mut self,
        cache_key: &str,
    ) -> Option<std::result::Result<DecodedGpuSource, String>> {
        let result = match self.pending_sources.get_mut(cache_key) {
            Some(receiver) => match receiver.try_recv() {
                std::result::Result::Ok(result) => Some(result),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Closed) => Some(Err("GPU source decoder stopped".to_owned())),
            },
            None => None,
        };

        if result.is_some() {
            self.pending_sources.remove(cache_key);
        }

        result
    }

    fn set_render_status(&self, render_key: &str, status: GpuSourceStatus) -> bool {
        set_render_status(&self.render_status, render_key, status)
    }

    fn set_terminal_render_status(
        &self,
        render_key: &str,
        status: GpuSourceStatus,
        repaint_ctx: &egui::Context,
    ) {
        publish_terminal_render_status(&self.render_status, render_key, status, repaint_ctx);
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'_>, render_key: &str) {
        let Some(photo) = self.renders.get(render_key) else {
            return;
        };

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &photo.bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }

    fn evict_old_sources(&mut self, protected_key: &str) {
        let capacity = GPU_SOURCE_CACHE_CAPACITY.max(1);
        while self.sources.len() > capacity {
            let Some(key) = self
                .sources
                .iter()
                .filter(|(key, _)| key.as_str() != protected_key)
                .min_by_key(|(_, photo)| photo.last_access)
                .map(|(key, _)| key.clone())
            else {
                break;
            };

            self.sources.remove(&key);
            let removed_render_keys = self
                .renders
                .iter()
                .filter(|(_, render)| render.source_key == key)
                .map(|(render_key, _)| render_key.clone())
                .collect::<Vec<_>>();
            self.renders.retain(|_, render| render.source_key != key);
            let mut render_status = self.render_status.lock();
            for render_key in removed_render_keys {
                render_status.remove(&render_key);
            }
        }
    }

    fn evict_old_renders(&mut self, protected_key: &str) {
        let capacity = GPU_RENDER_CACHE_CAPACITY.max(1);
        while self.renders.len() > capacity {
            let Some(key) = self
                .renders
                .iter()
                .filter(|(key, _)| key.as_str() != protected_key)
                .min_by_key(|(_, render)| render.last_access)
                .map(|(key, _)| key.clone())
            else {
                break;
            };

            self.renders.remove(&key);
            self.render_status.lock().remove(&key);
        }
    }
}

struct DecodedGpuSource {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl DecodedGpuSource {
    fn size(&self) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: self.width,
            height: self.height,
            depth_or_array_layers: 1,
        }
    }
}

fn spawn_gpu_source_decode(
    path: PathBuf,
    max_texture_side: u32,
    sender: oneshot::Sender<std::result::Result<DecodedGpuSource, String>>,
    repaint_ctx: egui::Context,
) {
    let decode = move || decode_gpu_source(path, max_texture_side);

    if let std::result::Result::Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(async move {
            let result = spawn_blocking(decode)
                .await
                .map_err(|error| error.to_string())
                .and_then(|result| result);
            let _ = sender.send(result);
            repaint_ctx.request_repaint();
        });
    } else {
        std::thread::spawn(move || {
            let _ = sender.send(decode());
            repaint_ctx.request_repaint();
        });
    }
}

fn decode_gpu_source(
    path: PathBuf,
    max_texture_side: u32,
) -> std::result::Result<DecodedGpuSource, String> {
    let image = decode_oriented_image(&path, max_texture_side)?;
    let rgba = image.to_rgba8();
    Ok(DecodedGpuSource {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

fn gpu_source_cache_key(source_key: &str, path: &Path, max_texture_side: u32) -> String {
    format!(
        "{}:{}:{}",
        source_key,
        max_texture_side,
        path_version_key(path)
    )
}

struct GpuSourceTexture {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    last_access: u64,
}

struct GpuAdjustedPhoto {
    source_key: String,
    curve_texture: wgpu::Texture,
    _curve_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    uniform: AdjustmentUniform,
    curve_lut: [u8; CURVE_LUT_BYTES],
    last_access: u64,
}

fn write_curve_lut(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    curve_lut: &[u8; CURVE_LUT_BYTES],
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        curve_lut,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * CURVE_LUT_SIZE),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: CURVE_LUT_SIZE,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn terminal_status_transition_requests_one_presentation_wakeup() {
        let statuses = Arc::new(Mutex::new(HashMap::new()));
        let ctx = egui::Context::default();
        let repaint_count = Arc::new(AtomicUsize::new(0));
        let callback_count = Arc::clone(&repaint_count);
        ctx.set_request_repaint_callback(move |_| {
            callback_count.fetch_add(1, Ordering::Relaxed);
        });

        publish_terminal_render_status(&statuses, "render", GpuSourceStatus::Ready, &ctx);
        assert_eq!(repaint_count.load(Ordering::Relaxed), 1);
        assert_eq!(statuses.lock().get("render"), Some(&GpuSourceStatus::Ready));

        publish_terminal_render_status(&statuses, "render", GpuSourceStatus::Ready, &ctx);
        assert_eq!(repaint_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn uniform_maps_clipped_rect_into_image_uv_space() {
        let image_rect = Rect::from_min_max(egui::pos2(100.0, 200.0), egui::pos2(300.0, 400.0));
        let callback_rect = Rect::from_min_max(egui::pos2(150.0, 250.0), egui::pos2(250.0, 350.0));
        let source_uv = Rect::from_min_max(egui::pos2(0.2, 0.1), egui::pos2(0.8, 0.9));

        let uniform = AdjustmentUniform::new(
            &PhotoAdjustments::default(),
            0.0,
            callback_rect,
            image_rect,
            source_uv,
        );

        assert_eq!(uniform.uv_min_x, 0.25);
        assert_eq!(uniform.uv_min_y, 0.25);
        assert_eq!(uniform.uv_span_x, 0.5);
        assert_eq!(uniform.uv_span_y, 0.5);
        assert_eq!(uniform.source_uv_min_x, 0.2);
        assert_eq!(uniform.source_uv_min_y, 0.1);
        assert!((uniform.source_uv_span_x - 0.6).abs() < f32::EPSILON);
        assert!((uniform.source_uv_span_y - 0.8).abs() < f32::EPSILON);
    }
}
