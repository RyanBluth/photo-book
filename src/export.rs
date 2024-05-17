use eframe::wgpu::naga::back;
use egui::epaint::util::FloatOrd;
use egui::load::{TextureLoadResult, TexturePoll};
use egui::util::History;
use egui::{Context, Pos2, Rect};
use log::{error, info};
use skia_safe::surfaces::raster_n32_premul;
use skia_safe::{EncodedImageFormat, Paint, Point};
use std::borrow::Borrow;
use std::default;
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, RwLock};
use tokio::signal::unix::Signal;
use tokio::task::spawn_blocking;

use smol_egui_skia::{draw_onto_canvas, rasterize, EguiSkia, RasterizeOptions};

use crate::dependencies::{Dependency, Singleton, SingletonFor};
use crate::history;
use crate::photo_manager::{self, PhotoManager};
use crate::scene::canvas_scene::CanvasHistoryManager;
use crate::widget::canvas_info::layers::LayerContent;
use crate::widget::page_canvas::{Canvas, CanvasState};

pub fn export(mut canvas_state: CanvasState, path: &str) {
    
    let path = String::from(path);
    
    spawn_blocking(move || {
        let size = canvas_state.page.size_pixels();
        canvas_state.zoom = 1.0;

        // let mut surface =
        //     raster_n32_premul((size.x as i32, size.y as i32)).expect("Failed to create surface");

        // let RasterizeOptions {
        //     pixels_per_point,
        //     frames_before_screenshot,
        // } = default::Default::default();
        // let mut backend = EguiSkia::new(pixels_per_point);
        // egui_extras::install_image_loaders(&backend.egui_ctx);

        let photo_manager = Singleton::new(PhotoManager::new());
        let mut history_manager = CanvasHistoryManager::new();

        let mut canvas = Canvas::with_photo_manager(
            photo_manager.clone(),
            &mut canvas_state,
            Rect::from_min_max(Pos2::ZERO, size.to_pos2()),
            &mut history_manager,
        );

        let mut surface = rasterize((size.x as i32, size.y as i32), |ctx|{

            info!("Exporting canvas to {}", path);

            egui_extras::install_image_loaders(&ctx);

            photo_manager.with_lock_mut(|photo_manager| {
                for layer in canvas.state.layers.values() {
                    match &layer.content {
                        LayerContent::Photo(photo)
                        | LayerContent::TemplatePhoto {
                            photo: Some(photo), ..
                        } => loop {
                            match photo_manager.texture_for_blocking(&photo.photo, &ctx) {
                                Ok(Some(_)) => {
                                    info!("Texture loaded for {}", photo.photo.uri());
                                    break;
                                }
                                Ok(None) => {
                                    continue;
                                }
                                Err(error) => {
                                    error!("Error loading texture: {:?}", error);
                                    break;
                                }
                            }
                        },
                        LayerContent::TemplatePhoto { photo: None, .. } => {}
                        LayerContent::Text(_) => {}
                        LayerContent::TemplateText { .. } => {}
                    }
                }
            });

            
            egui::CentralPanel::default().show(ctx, |ui| {
                canvas.show(ui);
            });
        }, Default::default());



        // let image_info = surface.canvas().image_info();

        // let input = egui::RawInput {
        //     screen_rect: Some(
        //         [
        //             Pos2::default(),
        //             Pos2::new(image_info.width() as f32, image_info.height() as f32),
        //         ]
        //         .into(),
        //     ),
        //     ..Default::default()
        // };

        // let mut output_surface: Option<_> = None;

        // for _ in 0..frames_before_screenshot {
        //     output_surface = Some(backend.run(input.clone(), |ctx: &egui::Context| {
        //         egui::CentralPanel::default().show(ctx, |ui| {
        //             canvas.show(ui);
        //         });
        //     }));
        // }
        // backend.paint(surface.canvas());

        let data = surface
            .image_snapshot()
            .encode_to_data(EncodedImageFormat::JPEG)
            .expect("Failed to encode image");

        File::create("output.jpg")
            .unwrap()
            .write_all(&data)
            .unwrap();
    });
}
