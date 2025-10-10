use egui::{Pos2, Rect};
use log::{error, info};

use skia_safe::EncodedImageFormat;
use skia_safe::surfaces::raster_n32_premul;

use printpdf::{Mm, Op, PdfDocument, PdfPage, PdfSaveOptions, RawImage, XObjectTransform};
use std::collections::HashMap;
use std::default;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::task::spawn_blocking;

use smol_egui_skia::{EguiSkia, RasterizeOptions};

use thiserror::Error;

use crate::dependencies::{Dependency, Singleton, SingletonFor};

use crate::font_manager::FontManager;
use crate::modal::basic::BasicModal;
use crate::modal::manager::{ModalManager, TypedModalId};
use crate::modal::progress::ProgressModal;
use crate::photo_manager::PhotoManager;
use crate::scene::canvas_scene::CanvasHistoryManager;
use crate::widget::canvas::{Canvas, CanvasState};
use crate::widget::canvas_info::layers::LayerContent;

#[derive(Error, Debug, Clone)]
pub enum ExportError {
    #[error("Failed to create surface")]
    SurfaceCreationError,
    #[error("Error loading texture: {0}")]
    TextureLoadingError(String),
    #[error("Failed to encode image")]
    ImageEncodingError,
    #[error("File operation error: {0}")]
    FileError(String),
    #[error("PDF rendering error: {0}")]
    PdfRenderingError(String),
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct ExportTaskId {
    pub task_id: u64,
}

#[derive(Debug, Clone)]
pub enum ExportTaskStatus {
    InProgress(f32),
    Completed,
    Failed(ExportError),
}

pub struct Exporter {
    pub tasks: Arc<Mutex<HashMap<ExportTaskId, ExportTaskStatus>>>,
}

impl Exporter {
    /// Get the maximum texture size from global GPU limits
    fn get_max_texture_size() -> usize {
        // Import the global limit from main module
        use crate::MAX_TEXTURE_SIZE;
        use std::sync::atomic::Ordering;

        let size = MAX_TEXTURE_SIZE.load(Ordering::Relaxed);
        if size > 0 {
            size as usize
        } else {
            // Fallback for export operations if GPU limits weren't set
            8192
        }
    }
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_task_status(&self, task_id: ExportTaskId) -> Option<ExportTaskStatus> {
        let tasks = self.tasks.lock().unwrap();
        tasks.get(&task_id).cloned()
    }

    pub fn export(
        &mut self,
        ctx: egui::Context,
        pages: Vec<CanvasState>,
        directory: PathBuf,
        file_name: &str,
    ) -> ExportTaskId {
        let task_id = ExportTaskId {
            task_id: rand::random(),
        };

        let tasks = self.tasks.clone();

        let file_name = file_name.to_string();

        if !directory.exists() {
            if let Err(err) = std::fs::create_dir_all(&directory) {
                let mut tasks = tasks.lock().unwrap();
                tasks.insert(
                    task_id,
                    ExportTaskStatus::Failed(ExportError::FileError(err.to_string())),
                );
                ctx.request_repaint();
                return task_id;
            }
        }

        spawn_blocking(move || {
            let modal_manager: Singleton<ModalManager> = Dependency::get();
            let progress_modal_id =
                ModalManager::push(ProgressModal::new("Exporting", "Preparing", "Cancel", 0.0));

            let show_export_failure_modal =
                |progress_modal_id: TypedModalId<ProgressModal>, error: ExportError| {
                    let modal_manager: Singleton<ModalManager> = Dependency::get();
                    _ = modal_manager.with_lock_mut(|modal_manager| {
                        modal_manager.dismiss(progress_modal_id);
                    });
                    ModalManager::push(BasicModal::new(
                        "Export Failed",
                        error.to_string(),
                        "Dismiss",
                    ));
                };

            let mut page_number = 0;
            let num_pages = pages.len();
            for page in &pages {
                if let Err(err) = Self::export_page(page.clone(), &directory, page_number) {
                    let mut tasks = tasks.lock().unwrap();
                    tasks.insert(task_id, ExportTaskStatus::Failed(err.clone()));
                    show_export_failure_modal(progress_modal_id, err);
                    ctx.request_repaint();
                    return;
                }
                page_number += 1;
                let progress = page_number as f32 / (num_pages as f32 + 1.0); // +1 for the PDF generation
                let mut tasks = tasks.lock().unwrap();
                tasks.insert(task_id, ExportTaskStatus::InProgress(progress));
                _ = modal_manager.with_lock_mut(|modal_manager| {
                    modal_manager.modify(&progress_modal_id, |progress_modal| {
                        progress_modal.progress = progress;
                        progress_modal.message =
                            format!("Exporting page {}/{}", page_number, num_pages);
                    })
                });

                ctx.request_repaint();
            }

            if let Err(err) = Self::export_pdf(&pages, &directory, &file_name) {
                let mut tasks: std::sync::MutexGuard<'_, HashMap<ExportTaskId, ExportTaskStatus>> =
                    tasks.lock().unwrap();
                tasks.insert(task_id, ExportTaskStatus::Failed(err.clone()));
                show_export_failure_modal(progress_modal_id, err);
                ctx.request_repaint();
                return;
            }

            let mut tasks = tasks.lock().unwrap();
            tasks.insert(task_id, ExportTaskStatus::Completed);
            modal_manager.with_lock_mut(|modal_manager| {
                modal_manager.dismiss(progress_modal_id);
            });
            ctx.request_repaint();
        });

        let mut tasks = self.tasks.lock().unwrap();
        tasks.insert(task_id, ExportTaskStatus::InProgress(0.0));

        task_id
    }

    fn export_page(
        mut canvas_state: CanvasState,
        directory: &PathBuf,
        page_number: u32,
    ) -> Result<(), ExportError> {
        /* */
        let directory = PathBuf::from(directory);

        let size = canvas_state.page.size_pixels();
        canvas_state.zoom = 1.0;

        let mut surface = raster_n32_premul((size.x as i32, size.y as i32))
            .ok_or(ExportError::SurfaceCreationError)?;

        let rasterize_options = RasterizeOptions {
            pixels_per_point: 1.0,
            frames_before_screenshot: 500,
        };
        let mut backend = EguiSkia::new(rasterize_options.pixels_per_point);
        egui_extras::install_image_loaders(&backend.egui_ctx);

        backend.egui_ctx.input_mut(|input| {
            input.max_texture_side = Self::get_max_texture_size();
        });

        let photo_manager = Singleton::new(PhotoManager::new());
        let mut history_manager = CanvasHistoryManager::preview();

        let mut canvas = Canvas::new(
            &mut canvas_state,
            Rect::from_min_max(Pos2::ZERO, size.to_pos2()),
            &mut history_manager,
        );

        photo_manager.with_lock_mut(|photo_manager| {
            for layer in canvas.state.layers.values() {
                match &layer.content {
                    LayerContent::Photo(photo)
                    | LayerContent::TemplatePhoto {
                        photo: Some(photo), ..
                    } => loop {
                        match photo_manager.texture_for_blocking(&photo.photo, &backend.egui_ctx) {
                            Ok(Some(_)) => {
                                info!("Texture loaded for {}", photo.photo.uri());
                                break;
                            }
                            Ok(None) => {
                                continue;
                            }
                            Err(error) => {
                                error!("Error loading texture: {:?}", error);
                                return Err(ExportError::TextureLoadingError(error.to_string()));
                            }
                        }
                    },
                    LayerContent::TemplatePhoto { photo: None, .. } => {}
                    LayerContent::Text(_) => {}
                    LayerContent::TemplateText { .. } => {}
                }
            }
            Ok(())
        })?;

        let font_manager: Singleton<FontManager> = Dependency::get();

        if let Some(font_definitions) =
            font_manager.with_lock(|font_manager| font_manager.font_definitions.clone())
        {
            backend.egui_ctx.set_fonts((*font_definitions).clone());
        };

        let image_info = surface.canvas().image_info();

        let input = egui::RawInput {
            screen_rect: Some(
                [
                    Pos2::default(),
                    Pos2::new(image_info.width() as f32, image_info.height() as f32),
                ]
                .into(),
            ),
            ..Default::default()
        };

        let mut _output_surface: Option<_> = None;
        for _ in 0..rasterize_options.frames_before_screenshot {
            _output_surface = Some(backend.run(input.clone(), |ctx: &egui::Context| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    canvas.show_preview(ui, Rect::from_min_max(Pos2::ZERO, size.to_pos2()));
                });
            }));
        }

        backend.paint(surface.canvas());

        let data = surface
            .image_snapshot()
            .encode_to_data(EncodedImageFormat::JPEG)
            .ok_or(ExportError::ImageEncodingError)?;

        let image_path = directory.join(format!("page_{}.jpg", page_number));

        let mut output_file =
            File::create(&image_path).map_err(|e| ExportError::FileError(e.to_string()))?;
        output_file
            .write_all(&data)
            .map_err(|e| ExportError::FileError(e.to_string()))?;

        Ok(())
    }

    fn export_pdf(
        pages: &Vec<CanvasState>,
        directory: &PathBuf,
        file_name: &str,
    ) -> Result<(), ExportError> {
        let directory = PathBuf::from(directory);

        let mut doc = PdfDocument::new(file_name);
        let mut pdf_pages = Vec::new();

        for page_number in 0..pages.len() {
            let image_path = directory.join(format!("page_{}.jpg", page_number));

            let page_size = pages[page_number].page.size_mm();
            let (mm_width, mm_height) = (Mm(page_size.x), Mm(page_size.y));

            // Load and decode the JPEG image
            let image_bytes =
                std::fs::read(&image_path).map_err(|e| ExportError::FileError(e.to_string()))?;
            let mut warnings = Vec::new();
            let image = RawImage::decode_from_bytes(&image_bytes, &mut warnings).map_err(|e| {
                ExportError::PdfRenderingError(format!("Error loading image: {:?}", e))
            })?;

            // Add image to document and get XObject ID
            let image_xobject_id = doc.add_image(&image);

            // Calculate transform based on DPI
            let dpi = pages[page_number].page.ppi() as f32;
            let transform = XObjectTransform {
                dpi: Some(dpi),
                ..Default::default()
            };

            // Create page operations
            let page_contents = vec![Op::UseXobject {
                id: image_xobject_id,
                transform,
            }];

            // Create the page
            let page = PdfPage::new(mm_width, mm_height, page_contents);
            pdf_pages.push(page);
        }

        // Add all pages to document and save
        let mut warnings = Vec::new();
        let pdf_bytes = doc
            .with_pages(pdf_pages)
            .save(&PdfSaveOptions::default(), &mut warnings);

        let mut pdf_path = directory.join(file_name);
        pdf_path.set_extension("pdf");

        let mut output_pdf =
            File::create(pdf_path).map_err(|e| ExportError::FileError(e.to_string()))?;

        output_pdf
            .write_all(&pdf_bytes)
            .map_err(|e| ExportError::FileError(e.to_string()))
    }
}
