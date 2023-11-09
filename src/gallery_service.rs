use eframe::egui::SizeHint::Size;
use eframe::egui::{self, Context, TextureOptions};
use eframe::epaint::util::{FloatOrd, OrderedFloat};
use log::info;

use std::fs::DirEntry;
use std::io::BufWriter;
use std::num::NonZeroU32;
use std::{fs::create_dir, path::PathBuf};

use anyhow::{anyhow, Ok};
use fr::CpuExtensions;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::io::Reader as ImageReader;
use image::{ColorType, ImageEncoder};

use fast_image_resize as fr;

use crate::dependencies::{
    DependencyFor, Singleton, SingletonFor,
};
use crate::image_cache;
use crate::photo_manager::PhotoManager;
use crate::{dependencies::Dependency, image_cache::ImageCache, utils};

const THUMBNAIL_SIZE: f32 = 256.0;

pub struct ThumbnailService {
    photo_manager: Singleton<PhotoManager>,
}

impl ThumbnailService {
    pub fn new() -> Self {
        Self {
            photo_manager: Dependency::<PhotoManager>::get(),
        }
    }

    pub fn gen_thumbnails(&self, dir: PathBuf, ctx: Context) -> anyhow::Result<()> {
        let path: PathBuf = PathBuf::from(dir);

        if !path.exists() {
            return Err(anyhow::anyhow!("Path 33 not exist"));
        }

        if !path.is_dir() {
            return Err(anyhow::anyhow!("Path is not a directory"));
        }

        let thumbnail_dir = path.join(".thumb");

        if !thumbnail_dir.exists() {
            info!("Creating thumbnail directory: {:?}", &thumbnail_dir);
            create_dir(&thumbnail_dir)?;
        }

        let entries = path.read_dir()?.collect::<Result<Vec<_>, _>>()?;
        let partitions = utils::partition_iterator(entries.into_iter(), 8);

        for partition in partitions {
            let thumbnail_dir = thumbnail_dir.clone();
            let photo_manager = self.photo_manager.clone();
            let ctx = ctx.clone();
            tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                partition.into_iter().try_for_each(|entry| {
                    Self::gen_thumbnail(
                        entry,
                        &thumbnail_dir,
                        &photo_manager,
                        &ctx,
                    )
                })?;
                Ok(())
            });
        }

        Ok(())
    }

    fn gen_thumbnail(
        entry: DirEntry,
        thumbnail_dir: &PathBuf,
        photo_manager: &Singleton<PhotoManager>,
        ctx: &Context,
    ) -> anyhow::Result<()> {
        let path = entry.path();

        let file_name = path.file_name();
        let extension = path.extension();

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

                let img = ImageReader::open(&path)?.decode()?;
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
