use egui::{Context, Pos2, Rect};

use skia_safe::EncodedImageFormat;
use skia_safe::surfaces::raster_n32_premul;

use printpdf::{Mm, Op, PdfDocument, PdfPage, PdfSaveOptions, RawImage, XObjectTransform};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::task::spawn_blocking;

use smol_egui_skia::EguiSkia;

use thiserror::Error;

use crate::font_manager::FontManager;
use crate::{dep, dep_mut};

use crate::cancellation::CancellationToken;
use crate::modal::basic::BasicModal;
use crate::modal::manager::ModalManager;
use crate::modal::progress::ProgressModal;
use crate::photo_manager::PhotoManager;
use crate::scene::canvas_scene::CanvasHistoryManager;
use crate::widget::canvas::types::CanvasPhoto;
use crate::widget::canvas::{Canvas, CanvasState};
use crate::widget::canvas_info::layers::{Layer, LayerContent};

#[derive(Error, Debug, Clone)]
#[allow(clippy::enum_variant_names)]
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
    #[error("Export cancelled")]
    Cancelled,
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
    Cancelled,
}

pub struct Exporter {
    pub tasks: Arc<Mutex<HashMap<ExportTaskId, ExportTaskStatus>>>,
}

struct AdoptedGlobalPhotoCache {
    ctx: Context,
}

impl Drop for AdoptedGlobalPhotoCache {
    fn drop(&mut self) {
        dep_mut!(PhotoManager, |photo_manager| {
            photo_manager.remove_cached_textures_for_context(&self.ctx);
        });
    }
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
        let cancellation = CancellationToken::new();

        if !directory.exists()
            && let Err(err) = std::fs::create_dir_all(&directory)
        {
            let mut tasks = tasks.lock().unwrap();
            tasks.insert(
                task_id,
                ExportTaskStatus::Failed(ExportError::FileError(err.to_string())),
            );
            ctx.request_repaint();
            return task_id;
        }

        spawn_blocking(move || {
            let progress_modal_id = ModalManager::push(
                ProgressModal::new("Exporting", "Preparing", "Cancel", 0.0)
                    .with_cancellation(cancellation.clone()),
            );

            let export_result = (|| -> Result<(), ExportError> {
                let num_pages = pages.len();
                for (page_index, page) in pages.iter().enumerate() {
                    check_cancelled(&cancellation)?;
                    Self::export_page(page.clone(), &directory, page_index as u32, &cancellation)?;

                    let page_number = page_index + 1;
                    let progress = page_number as f32 / (num_pages as f32 + 1.0);
                    tasks
                        .lock()
                        .unwrap()
                        .insert(task_id, ExportTaskStatus::InProgress(progress));
                    _ = dep_mut!(ModalManager, |modal_manager| {
                        modal_manager.modify(&progress_modal_id, |progress_modal| {
                            progress_modal.progress = progress;
                            progress_modal.message =
                                format!("Exporting page {page_number}/{num_pages}");
                        })
                    });
                    ctx.request_repaint();
                }

                check_cancelled(&cancellation)?;
                Self::export_pdf(&pages, &directory, &file_name, &cancellation)?;
                check_cancelled(&cancellation)
            })();

            let task_status = match &export_result {
                Ok(()) => ExportTaskStatus::Completed,
                Err(ExportError::Cancelled) => ExportTaskStatus::Cancelled,
                Err(error) => ExportTaskStatus::Failed(error.clone()),
            };
            tasks.lock().unwrap().insert(task_id, task_status);

            dep_mut!(ModalManager, |modal_manager| {
                modal_manager.dismiss(progress_modal_id);
            });
            if let Err(error) = export_result
                && !matches!(error, ExportError::Cancelled)
            {
                ModalManager::push(BasicModal::new(
                    "Export Failed",
                    error.to_string(),
                    "Dismiss",
                ));
            }
            ctx.request_repaint();
        });

        let mut tasks = self.tasks.lock().unwrap();
        tasks.insert(task_id, ExportTaskStatus::InProgress(0.0));

        task_id
    }

    #[allow(deprecated)]
    fn export_page(
        mut canvas_state: CanvasState,
        directory: &Path,
        page_number: u32,
        cancellation: &CancellationToken,
    ) -> Result<(), ExportError> {
        let directory = PathBuf::from(directory);

        let size = canvas_state.page.size_pixels();
        canvas_state.zoom = 1.0;

        let mut surface = raster_n32_premul((size.x as i32, size.y as i32))
            .ok_or(ExportError::SurfaceCreationError)?;

        let mut backend = EguiSkia::new(1.0);
        egui_extras::install_image_loaders(&backend.egui_ctx);

        backend.egui_ctx.input_mut(|input| {
            input.max_texture_side = Self::get_max_texture_size();
        });

        let mut history_manager = CanvasHistoryManager::preview();

        let mut canvas = Canvas::new(
            &mut canvas_state,
            Rect::from_min_max(Pos2::ZERO, size.to_pos2()),
            &mut history_manager,
        )
        .gpu_photo_adjustments(false);

        let mut export_photo_manager = PhotoManager::new();
        let prepared_photo_count =
            match prepare_visible_photo_textures_for_export(canvas.state.layers.values(), |photo| {
                export_photo_manager.texture_for_export_cancellable(
                    &photo.photo,
                    &photo.adjustments,
                    &backend.egui_ctx,
                    Some(cancellation),
                )
            }) {
                Err(_) if cancellation.is_cancelled() => return Err(ExportError::Cancelled),
                result => result?,
            };

        let cache_adopted = dep_mut!(PhotoManager, |photo_manager| {
            export_photo_manager
                .transfer_cached_textures_for_context_to(&backend.egui_ctx, photo_manager)
        });
        if prepared_photo_count > 0 && !cache_adopted {
            return Err(ExportError::TextureLoadingError(
                "Export texture preparation completed without a transferable context cache"
                    .to_owned(),
            ));
        }
        let adopted_cache = cache_adopted.then(|| AdoptedGlobalPhotoCache {
            ctx: backend.egui_ctx.clone(),
        });

        if let Some(font_definitions) = dep!(FontManager, |font_manager| font_manager
            .font_definitions
            .clone())
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

        backend.paint_when_ready_ui(
            surface.canvas(),
            input.clone(),
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    canvas.show_preview(ui, Rect::from_min_max(Pos2::ZERO, size.to_pos2()));
                });
            },
            Some(usize::MAX),
        );
        backend.paint(surface.canvas());
        drop(adopted_cache);

        let data = surface
            .image_snapshot()
            .encode(None, EncodedImageFormat::JPEG, 100)
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
        pages: &[CanvasState],
        directory: &Path,
        file_name: &str,
        cancellation: &CancellationToken,
    ) -> Result<(), ExportError> {
        let directory = PathBuf::from(directory);

        let mut doc = PdfDocument::new(file_name);
        let mut pdf_pages = Vec::new();

        for_each_cancellable(
            pages.iter().enumerate(),
            cancellation,
            |(page_number, page)| {
                let image_path = directory.join(format!("page_{}.jpg", page_number));

                let page_size = page.page.size_mm();
                let (mm_width, mm_height) = (Mm(page_size.x), Mm(page_size.y));

                // Load and decode the JPEG image
                let image_bytes = std::fs::read(&image_path)
                    .map_err(|e| ExportError::FileError(e.to_string()))?;
                let mut warnings = Vec::new();
                let image =
                    RawImage::decode_from_bytes(&image_bytes, &mut warnings).map_err(|e| {
                        ExportError::PdfRenderingError(format!("Error loading image: {:?}", e))
                    })?;

                // Add image to document and get XObject ID
                let image_xobject_id = doc.add_image(&image);

                // Calculate transform based on DPI
                let dpi = page.page.ppi() as f32;
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
                Ok(())
            },
        )?;

        // Add all pages to document and save
        check_cancelled(cancellation)?;
        let mut warnings = Vec::new();
        let pdf_bytes = doc
            .with_pages(pdf_pages)
            .save(&PdfSaveOptions::default(), &mut warnings);
        check_cancelled(cancellation)?;

        let mut pdf_path = directory.join(file_name);
        pdf_path.set_extension("pdf");

        let mut output_pdf =
            File::create(pdf_path).map_err(|e| ExportError::FileError(e.to_string()))?;

        output_pdf
            .write_all(&pdf_bytes)
            .map_err(|e| ExportError::FileError(e.to_string()))?;
        check_cancelled(cancellation)
    }
}

fn check_cancelled(cancellation: &CancellationToken) -> Result<(), ExportError> {
    if cancellation.is_cancelled() {
        Err(ExportError::Cancelled)
    } else {
        Ok(())
    }
}

fn for_each_cancellable<I>(
    items: impl IntoIterator<Item = I>,
    cancellation: &CancellationToken,
    mut process: impl FnMut(I) -> Result<(), ExportError>,
) -> Result<(), ExportError> {
    for item in items {
        check_cancelled(cancellation)?;
        process(item)?;
    }
    check_cancelled(cancellation)
}

fn prepare_visible_photo_textures_for_export<'a, T>(
    layers: impl IntoIterator<Item = &'a Layer>,
    mut load_texture: impl FnMut(&CanvasPhoto) -> anyhow::Result<T>,
) -> Result<usize, ExportError> {
    let mut loaded_count = 0;
    for photo in layers.into_iter().filter_map(visible_photo_for_export) {
        match load_texture(photo) {
            Ok(_) => loaded_count += 1,
            Err(error) => {
                return Err(ExportError::TextureLoadingError(format!(
                    "{}: {error}",
                    photo.photo.uri()
                )));
            }
        }
    }

    Ok(loaded_count)
}

fn visible_photo_for_export(layer: &Layer) -> Option<&CanvasPhoto> {
    if !layer.visible {
        return None;
    }

    match &layer.content {
        LayerContent::Photo(photo)
        | LayerContent::TemplatePhoto {
            photo: Some(photo), ..
        } => Some(photo),
        LayerContent::TemplatePhoto { photo: None, .. }
        | LayerContent::Text(_)
        | LayerContent::TemplateText { .. }
        | LayerContent::Shape(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::photo_adjustments::PhotoAdjustments;
    use crate::photo::{MetadataCollection, Photo, PhotoMetadata};

    fn photo_layer(path: &str, visible: bool) -> Layer {
        let photo = Photo {
            path: PathBuf::from(path),
            metadata: PhotoMetadata {
                fields: MetadataCollection::new(),
            },
            thumbnail_hash: "export-test".to_owned(),
            last_modified: None,
        };
        let mut layer = Layer::new_text_layer();
        layer.content = LayerContent::Photo(CanvasPhoto {
            photo,
            adjustments: PhotoAdjustments::default(),
            crop: Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        });
        layer.visible = visible;
        layer
    }

    #[test]
    fn prepares_only_visible_photo_layers_for_export() {
        let visible = photo_layer("/visible.jpg", true);
        let hidden = photo_layer("/hidden.jpg", false);
        let non_photo = Layer::new_text_layer();
        let layers = [&visible, &hidden, &non_photo];
        let mut loaded_paths = Vec::new();

        let loaded_count = prepare_visible_photo_textures_for_export(layers, |photo| {
            loaded_paths.push(photo.photo.path.clone());
            Ok(())
        })
        .unwrap();

        assert_eq!(loaded_count, 1);
        assert_eq!(loaded_paths, [PathBuf::from("/visible.jpg")]);
    }

    #[test]
    fn cancellation_inside_pdf_page_loop_stops_before_the_next_page() {
        let cancellation = CancellationToken::new();
        let mut processed = Vec::new();

        let result = for_each_cancellable(0..3, &cancellation, |page| {
            processed.push(page);
            if page == 0 {
                cancellation.cancel();
            }
            Ok(())
        });

        assert!(matches!(result, Err(ExportError::Cancelled)));
        assert_eq!(processed, [0]);
    }

    #[test]
    fn texture_loader_errors_are_returned_as_export_errors() {
        let layer = photo_layer("/broken.jpg", true);

        let error = prepare_visible_photo_textures_for_export([&layer], |_| {
            Err::<(), _>(anyhow::anyhow!("decode failed"))
        })
        .unwrap_err();

        assert!(matches!(
            error,
            ExportError::TextureLoadingError(message)
                if message.contains("file:///broken.jpg") && message.contains("decode failed")
        ));
    }
}
