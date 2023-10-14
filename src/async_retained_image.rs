use std::path::PathBuf;

use eframe::{
    egui::{self, Image, Response, ScrollArea, Sense, Ui, Widget},
    epaint::{
        ahash::{HashMap, HashMapExt},
        Color32, Pos2, Rect, Rounding, Shape, Vec2,
    },
};
use url::Url;

use crate::gallery_service::GalleryService;

pub struct AsyncRetainedImage {
    path: PathBuf,
}

impl AsyncRetainedImage {
    pub fn new(path: PathBuf) -> Self {
        let file_name = path.file_name().unwrap().to_str().unwrap().to_string();
        let mut path = path;

        // path.pop();
        // path.push(".thumb");
        // path.push(file_name);

        Self { path }
    }
}

impl Widget for AsyncRetainedImage {
    fn ui(self, ui: &mut Ui) -> Response {
        let size = Vec2 { x: 256.0, y: 256.0 };

        let (rect, response) = ui.allocate_exact_size(
            size,
            Sense {
                click: false,
                drag: false,
                focusable: false,
            },
        );

        match GalleryService::thumbnail_path(self.path) {
            Some(thumbnail_path) => {
                ui.add(
                    Image::from_uri(format!("file://{}", thumbnail_path.as_os_str().to_str().unwrap()))
                        .fit_to_exact_size(size),
                );
            }
            None => {
                ui.painter().rect_filled(
                    rect,
                    Rounding {
                        nw: 3.0,
                        ne: 3.0,
                        sw: 3.0,
                        se: 3.0,
                    },
                    Color32::from_rgb(50, 50, 50),
                );
            }
        }

        response
    }
}
