use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::UNIX_EPOCH,
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

use crate::{model::photo_adjustments::PhotoAdjustments, photo::Photo};

const CURVE_LUT_SIZE: u32 = 256;
const CURVE_LUT_BYTES: usize = CURVE_LUT_SIZE as usize * 4;
const GPU_SOURCE_CACHE_CAPACITY: usize = 12;

#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
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
}

impl AdjustmentUniform {
    fn new(
        adjustments: &PhotoAdjustments,
        rotation_radians: f32,
        callback_rect: Rect,
        image_rect: Rect,
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
            rotation_radians,
            uv_min_x: uv_min.x,
            uv_min_y: uv_min.y,
            uv_span_x: uv_max.x - uv_min.x,
            uv_span_y: uv_max.y - uv_min.y,
        }
    }
}

#[derive(Debug)]
pub struct GpuPhotoAdjustmentRenderer {
    enabled: bool,
    source_status: Arc<Mutex<HashMap<String, GpuSourceStatus>>>,
}

impl Default for GpuPhotoAdjustmentRenderer {
    fn default() -> Self {
        Self {
            enabled: false,
            source_status: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GpuSourceStatus {
    Pending,
    Ready,
    Failed,
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
                targets: &[Some(render_state.target_format.into())],
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
                cache: HashMap::new(),
                pending_sources: HashMap::new(),
                source_status: self.source_status.clone(),
                access_counter: 0,
            });

        self.enabled = true;
    }

    pub fn try_paint(
        &self,
        ui: &mut egui::Ui,
        photo: &Photo,
        clip_rect: Rect,
        rect: Rect,
        adjustments: &PhotoAdjustments,
    ) -> bool {
        if !self.enabled || !adjustments.gpu_preview_supported() || !photo.path.exists() {
            return false;
        }

        self.try_paint_source(
            ui,
            &photo.path,
            photo.uri(),
            0.0,
            clip_rect,
            rect,
            adjustments,
            ui.ctx().input(|input| input.max_texture_side as u32),
        )
    }

    pub fn try_paint_thumbnail(
        &self,
        ui: &mut egui::Ui,
        photo: &Photo,
        clip_rect: Rect,
        rect: Rect,
        adjustments: &PhotoAdjustments,
    ) -> bool {
        if !self.enabled || adjustments.is_identity() || !adjustments.gpu_preview_supported() {
            return false;
        }

        let Ok(path) = photo.thumbnail_path() else {
            return false;
        };
        if !path.exists() {
            return false;
        }

        self.try_paint_source(
            ui,
            &path,
            photo.thumbnail_uri(),
            0.0,
            clip_rect,
            rect,
            adjustments,
            0,
        )
    }

    fn try_paint_source(
        &self,
        ui: &mut egui::Ui,
        path: &PathBuf,
        source_key: String,
        rotation_radians: f32,
        clip_rect: Rect,
        rect: Rect,
        adjustments: &PhotoAdjustments,
        max_texture_side: u32,
    ) -> bool {
        let callback_rect = clip_rect.intersect(rect);
        if !callback_rect.is_positive() {
            return false;
        }

        let source_key = gpu_source_cache_key(&source_key, path, max_texture_side);
        let status = self.source_status.lock().get(&source_key).copied();
        if status == Some(GpuSourceStatus::Failed) {
            return false;
        }

        if status.is_none() {
            self.source_status
                .lock()
                .insert(source_key.clone(), GpuSourceStatus::Pending);
        }

        ui.painter()
            .add(Shape::Callback(egui_wgpu::Callback::new_paint_callback(
                callback_rect,
                PhotoAdjustmentCallback {
                    path: path.clone(),
                    source_key: source_key.clone(),
                    max_texture_side,
                    uniform: AdjustmentUniform::new(
                        adjustments,
                        rotation_radians,
                        callback_rect,
                        rect,
                    ),
                    curve_lut: adjustments.curve_lut_rgba8(),
                },
            )));

        let ready = status == Some(GpuSourceStatus::Ready);
        if !ready {
            ui.ctx().request_repaint();
        }
        ready
    }
}

struct PhotoAdjustmentCallback {
    path: PathBuf,
    source_key: String,
    max_texture_side: u32,
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
            },
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

        resources.paint(render_pass, &self.source_key);
    }
}

struct PhotoAdjustmentRenderResources {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    cache: HashMap<String, GpuAdjustedPhoto>,
    pending_sources:
        HashMap<String, oneshot::Receiver<std::result::Result<DecodedGpuSource, String>>>,
    source_status: Arc<Mutex<HashMap<String, GpuSourceStatus>>>,
    access_counter: u64,
}

struct GpuSourceRequest<'a> {
    cache_key: &'a str,
    path: &'a PathBuf,
    max_texture_side: u32,
}

impl PhotoAdjustmentRenderResources {
    fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: GpuSourceRequest<'_>,
        uniform: AdjustmentUniform,
        curve_lut: &[u8],
    ) {
        self.access_counter = self.access_counter.saturating_add(1);
        let access = self.access_counter;

        if let Some(photo) = self.cache.get_mut(source.cache_key) {
            photo.last_access = access;
            queue.write_buffer(&photo.uniform_buffer, 0, bytemuck::bytes_of(&uniform));
            write_curve_lut(queue, &photo.curve_texture, curve_lut);
            self.set_source_status(source.cache_key, GpuSourceStatus::Ready);
            return;
        }

        let Some(decoded) = self.poll_or_start_decode(&source) else {
            self.set_source_status(source.cache_key, GpuSourceStatus::Pending);
            return;
        };

        let decoded = match decoded {
            Ok(decoded) => decoded,
            Err(error) => {
                error!("Failed to prepare GPU photo adjustment source: {error}");
                self.set_source_status(source.cache_key, GpuSourceStatus::Failed);
                return;
            }
        };

        let size = decoded.size();
        if size.width == 0 || size.height == 0 {
            self.set_source_status(source.cache_key, GpuSourceStatus::Failed);
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
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("photo adjustment uniform buffer"),
            contents: bytemuck::bytes_of(&uniform),
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
        write_curve_lut(queue, &curve_texture, curve_lut);
        let curve_view = curve_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("photo adjustment bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
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

        self.cache.insert(
            source.cache_key.to_string(),
            GpuAdjustedPhoto {
                _texture: texture,
                _view: view,
                curve_texture,
                _curve_view: curve_view,
                bind_group,
                uniform_buffer,
                last_access: access,
            },
        );
        self.evict_old_sources(source.cache_key);
        self.set_source_status(source.cache_key, GpuSourceStatus::Ready);
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
            spawn_gpu_source_decode(source.path.clone(), source.max_texture_side, sender);
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

    fn set_source_status(&self, cache_key: &str, status: GpuSourceStatus) {
        self.source_status
            .lock()
            .insert(cache_key.to_owned(), status);
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'_>, cache_key: &str) {
        let Some(photo) = self.cache.get(cache_key) else {
            return;
        };

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &photo.bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }

    fn evict_old_sources(&mut self, protected_key: &str) {
        let capacity = GPU_SOURCE_CACHE_CAPACITY.max(1);
        while self.cache.len() > capacity {
            let Some(key) = self
                .cache
                .iter()
                .filter(|(key, _)| key.as_str() != protected_key)
                .min_by_key(|(_, photo)| photo.last_access)
                .map(|(key, _)| key.clone())
            else {
                break;
            };

            self.cache.remove(&key);
            self.source_status.lock().remove(&key);
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
) {
    let decode = move || decode_gpu_source(path, max_texture_side);

    if let std::result::Result::Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(async move {
            let result = spawn_blocking(decode)
                .await
                .map_err(|error| error.to_string())
                .and_then(|result| result);
            let _ = sender.send(result);
        });
    } else {
        std::thread::spawn(move || {
            let _ = sender.send(decode());
        });
    }
}

fn decode_gpu_source(
    path: PathBuf,
    max_texture_side: u32,
) -> std::result::Result<DecodedGpuSource, String> {
    let file_bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    let format = image::guess_format(&file_bytes).map_err(|error| error.to_string())?;
    let reader = image::ImageReader::with_format(std::io::Cursor::new(file_bytes), format);
    let mut decoder = reader.into_decoder().map_err(|error| error.to_string())?;
    let orientation =
        image::ImageDecoder::orientation(&mut decoder).map_err(|error| error.to_string())?;
    let mut image =
        image::DynamicImage::from_decoder(decoder).map_err(|error| error.to_string())?;
    image.apply_orientation(orientation);

    if max_texture_side > 0
        && (image.width() > max_texture_side || image.height() > max_texture_side)
    {
        image = image.resize(
            max_texture_side,
            max_texture_side,
            image::imageops::FilterType::Triangle,
        );
    }

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

fn path_version_key(path: &Path) -> String {
    let Ok(metadata) = path.metadata() else {
        return "missing".to_owned();
    };

    let modified = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    format!("{}:{}", metadata.len(), modified)
}

struct GpuAdjustedPhoto {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    curve_texture: wgpu::Texture,
    _curve_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    last_access: u64,
}

fn write_curve_lut(queue: &wgpu::Queue, texture: &wgpu::Texture, curve_lut: &[u8]) {
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
