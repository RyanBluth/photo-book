use std::{
    collections::{HashMap, HashSet},
    fs::read_dir,
    io::BufWriter,
    path::PathBuf,
};

use backtrace::Backtrace;
use eframe::{
    egui::{
        load::{SizedTexture, TextureLoader},
        Context,
    },
    epaint::util::FloatOrd,
};
use image::{
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
    ColorType,
};
use log::{error, info};
use rayon::prelude::{ParallelBridge, ParallelIterator};

use crate::{
    dependencies::{Dependency, DependencyFor},
    photo::Photo,
};

use eframe::egui::SizeHint::Size;
use eframe::egui::{self, TextureOptions};

use std::fs::create_dir;
use std::fs::DirEntry;
use std::num::NonZeroU32;

use anyhow::{anyhow, Ok};
use fr::CpuExtensions;
use image::io::Reader as ImageReader;
use image::ImageEncoder;

use fast_image_resize as fr;

use crate::dependencies::{Singleton, SingletonFor};
use crate::image_cache;

use crate::{image_cache::ImageCache, utils};

const THUMBNAIL_SIZE: f32 = 256.0;

pub struct PhotoManager {
    pub photos: Vec<Photo>,

    texture_cache: HashMap<String, SizedTexture>,
    pending_textures: HashSet<String>,
}

impl PhotoManager {
    pub fn new() -> Self {
        Self {
            photos: Vec::new(),
            texture_cache: HashMap::new(),
            pending_textures: HashSet::new(),
        }
    }

    pub fn load_directory(&mut self, path: &PathBuf, ctx: &Context) -> anyhow::Result<()> {
        let entries: Vec<Result<std::fs::DirEntry, std::io::Error>> =
            read_dir(path).unwrap().collect();

        for entry in entries {
            let entry = entry.as_ref().unwrap();
            let path = entry.path();

            if path.extension().unwrap_or_default().to_ascii_lowercase() != "jpg" {
                continue;
            }
            self.photos.push(Photo::new(path));
        }

        self.gen_thumbnails(path.clone(), ctx.clone())
    }

    pub fn thumbnail_texture_for(
        &mut self,
        photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        if !photo.thumbnail_path()?.exists() {
            return Ok(None);
        }

        Self::load_texture(
            &photo.thumbnail_uri(),
            &ctx,
            &mut self.texture_cache,
            &mut self.pending_textures,
        )
    }

    pub fn tumbnail_texture_at(
        &mut self,
        at: usize,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        match self.photos.get(at) {
            Some(photo) => {
                if !photo.thumbnail_path()?.exists() {
                    return Ok(None);
                }
                Self::load_texture(
                    &photo.thumbnail_uri(),
                    &ctx,
                    &mut self.texture_cache,
                    &mut self.pending_textures,
                )
            }
            None => Ok(None),
        }
    }

    pub fn texture_for(
        &mut self,
        photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        Self::load_texture(
            &photo.uri(),
            &ctx,
            &mut self.texture_cache,
            &mut self.pending_textures,
        )
    }

    pub fn texture_at(&mut self, at: usize, ctx: &Context) -> anyhow::Result<Option<SizedTexture>> {
        match self.photos.get(at) {
            Some(photo) => Self::load_texture(
                &photo.uri(),
                &ctx,
                &mut self.texture_cache,
                &mut self.pending_textures,
            ),
            None => Ok(None),
        }
    }

    pub fn next_photo(
        &mut self,
        current_index: usize,
        ctx: &Context,
    ) -> anyhow::Result<Option<(Photo, usize)>> {
        let next_index = (current_index + 1) % self.photos.len();
        match self.photos.get(next_index) {
            Some(next_photo) => {
                if let Some(current_photo) = self.photos.get(current_index) {
                    if let Some(texture) = self.texture_cache.remove(&current_photo.uri()) {
                        info!("Freeing texture for photo {}", current_photo.uri());
                        ctx.forget_image(&current_photo.uri());
                        ctx.tex_manager().write().free(texture.id);
                    }
                }

                Ok(Some((next_photo.clone(), next_index)))
            }
            None => Ok(None),
        }
    }

    pub fn previous_photo(
        &mut self,
        current_index: usize,
        ctx: &Context,
    ) -> anyhow::Result<Option<(Photo, usize)>> {
        let prev_index = (current_index + self.photos.len() - 1) % self.photos.len();
        match self.photos.get(prev_index) {
            Some(previous_photo) => {
                if let Some(current_photo) = self.photos.get(current_index) {
                    if let Some(texture) = self.texture_cache.remove(&current_photo.uri()) {
                        info!("Freeing texture for photo {}", current_photo.uri());
                        ctx.forget_image(&current_photo.uri());
                        ctx.tex_manager().write().free(texture.id);
                    }
                }

                Ok(Some((previous_photo.clone(), prev_index)))
            }
            None => Ok(None),
        }
    }

    fn load_texture(
        uri: &str,
        ctx: &Context,
        texture_cache: &mut HashMap<String, SizedTexture>,
        pending_textures: &mut HashSet<String>,
    ) -> anyhow::Result<Option<SizedTexture>> {
        match texture_cache.get(uri) {
            Some(texture) => {
                pending_textures.remove(uri);
                Ok(Some(texture.clone()))
            }
            None => {
                let texture = ctx.try_load_texture(
                    uri,
                    eframe::egui::TextureOptions::default(),
                    eframe::egui::SizeHint::Scale(1.0_f32.ord()),
                )?;
                match texture {
                    eframe::egui::load::TexturePoll::Pending { size: _ } => {
                        pending_textures.insert(uri.to_string());
                        Ok(None)
                    }
                    eframe::egui::load::TexturePoll::Ready { texture } => {
                        texture_cache.insert(uri.to_string(), texture.clone());
                        Ok(Some(texture))
                    }
                }
            }
        }
    }

    fn gen_thumbnails(&self, dir: PathBuf, ctx: Context) -> anyhow::Result<()> {
        let path: PathBuf = PathBuf::from(dir);

        if !path.exists() {
            return Err(anyhow::anyhow!("Path does not exist"));
        }

        if !path.is_dir() {
            return Err(anyhow::anyhow!("Path is not a directory"));
        }

        let thumbnail_dir = path.join(".thumb");

        if !thumbnail_dir.exists() {
            info!("Creating thumbnail directory: {:?}", &thumbnail_dir);
            create_dir(&thumbnail_dir)?;
        }

        let partitions = utils::partition_iterator(self.photos.clone().into_iter(), 8);

        for partition in partitions {
            let thumbnail_dir = thumbnail_dir.clone();
            let ctx = ctx.clone();
            tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                partition.into_iter().for_each(|photo| {
                    let res = Self::gen_thumbnail(photo, &thumbnail_dir, &ctx);
                    if res.is_err() {
                        error!("{:?}", res);
                    }
                });

                Ok(())
            });
        }

        Ok(())
    }

    fn gen_thumbnail(photo: Photo, thumbnail_dir: &PathBuf, ctx: &Context) -> anyhow::Result<()> {
        let file_name = photo.path.file_name();
        let extension = photo.path.extension();

        if let (Some(file_name), Some(extension)) = (file_name, extension) {
            if extension.to_ascii_lowercase() == "jpg"
                || extension.to_ascii_lowercase() == "png"
                || extension.to_ascii_lowercase() == "jpeg"
            {
                let thumbnail_path = thumbnail_dir.join(file_name);

                if thumbnail_path.exists() {
                    info!("Thumbnail already exists: {:?}", &thumbnail_path);

                    let tex_result = ctx.try_load_texture(
                        &format!("file://{}", thumbnail_path.to_str().unwrap()),
                        TextureOptions {
                            magnification: egui::TextureFilter::Linear,
                            minification: egui::TextureFilter::Linear,
                        },
                        egui::SizeHint::Scale(1.0_f32.ord()),
                    )?;

                    ctx.request_repaint();

                    return Ok(());
                } else {
                    info!("Generating thumbnail: {:?}", &thumbnail_path);
                }

                let img = ImageReader::open(&photo.path)?.decode()?;
                let width = NonZeroU32::new(img.width()).ok_or(anyhow!("Invalid image width"))?;
                let height =
                    NonZeroU32::new(img.height()).ok_or(anyhow!("Invalid image height"))?;
                let mut src_image = fr::Image::from_vec_u8(
                    width,
                    height,
                    img.to_rgba8().into_raw(),
                    fr::PixelType::U8x4,
                )?;

                // Multiple RGB channels of source image by alpha channel
                // (not required for the Nearest algorithm)
                let alpha_mul_div = fr::MulDiv::default();
                alpha_mul_div.multiply_alpha_inplace(&mut src_image.view_mut())?;

                let ratio = img.height() as f32 / img.width() as f32;
                let dst_height: u32 = (THUMBNAIL_SIZE * ratio) as u32;

                let dst_width = NonZeroU32::new(THUMBNAIL_SIZE as u32)
                    .ok_or(anyhow!("Invalid destination image width"))?;
                let dst_height = NonZeroU32::new(dst_height)
                    .ok_or(anyhow!("Invalid destination image height"))?;
                let mut dst_image = fr::Image::new(dst_width, dst_height, src_image.pixel_type());

                // Get mutable view of destination image data
                let mut dst_view = dst_image.view_mut();

                // Create Resizer instance and resize source image
                // into buffer of destination image
                let mut resizer = fr::Resizer::new(fr::ResizeAlg::Nearest);

                let mut cpu_extensions_vec = vec![CpuExtensions::None];

                #[cfg(target_arch = "x86_64")]
                {
                    cpu_extensions_vec.push(CpuExtensions::Sse4_1);
                    cpu_extensions_vec.push(CpuExtensions::Avx2);
                }
                #[cfg(target_arch = "aarch64")]
                {
                    cpu_extensions_vec.push(CpuExtensions::Neon);
                }
                #[cfg(target_arch = "wasm32")]
                {
                    cpu_extensions_vec.push(CpuExtensions::Simd128);
                }

                for cpu_extension in cpu_extensions_vec {
                    if cpu_extension.is_supported() {
                        unsafe {
                            resizer.set_cpu_extensions(cpu_extension);
                            break;
                        }
                    }
                }

                resizer.resize(&src_image.view(), &mut dst_view)?;

                // Divide RGB channels of destination image by alpha
                alpha_mul_div.divide_alpha_inplace(&mut dst_view)?;

                // Write destination image as PNG-file
                let mut result_buf = BufWriter::new(Vec::new());

                match extension
                    .to_ascii_lowercase()
                    .to_str()
                    .ok_or(anyhow!("Failed to convert extension to str"))?
                {
                    "jpg" | "jpeg" => {
                        JpegEncoder::new(&mut result_buf).write_image(
                            dst_image.buffer(),
                            dst_width.get(),
                            dst_height.get(),
                            ColorType::Rgba8,
                        )?;
                    }
                    "png" => {
                        PngEncoder::new(&mut result_buf).write_image(
                            dst_image.buffer(),
                            dst_width.get(),
                            dst_height.get(),
                            ColorType::Rgba8,
                        )?;
                    }
                    _ => {
                        return Err(anyhow::anyhow!("Invalid file extension"));
                    }
                }

                let buf = result_buf.into_inner()?;
                std::fs::write(&thumbnail_path, &buf)?;

                info!("Thumbnail generated: {:?}", &thumbnail_path);

                let tex_result = ctx.try_load_texture(
                    &format!("file://{}", thumbnail_path.to_str().unwrap()),
                    TextureOptions {
                        magnification: egui::TextureFilter::Linear,
                        minification: egui::TextureFilter::Linear,
                    },
                    Size(u32::from(dst_width), u32::from(dst_height as NonZeroU32)),
                )?;

                ctx.request_repaint();
            }
        }

        Ok(())
    }
}
