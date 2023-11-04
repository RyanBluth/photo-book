use eframe::egui::Widget;

use crate::photo::Photo;

pub struct PhotoInfo<'a> {
    pub photo: &'a Photo,
}

impl<'a> PhotoInfo<'a> {
    pub fn new(photo: &'a Photo) -> Self {
        Self { photo }
    }
}

impl<'a> Widget for PhotoInfo<'a> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> eframe::egui::Response {
        ui.allocate_ui(ui.available_size(), |ui| {
            ui.label(self.photo.file_name());
            ui.label(format!("Path: {}", self.photo.string_path()));
            ui.label(format!("URI: {}", self.photo.uri()));
            ui.label(format!("Thumbnail URI: {}", self.photo.thumbnail_uri()));
            ui.label(format!(
                "Thumbnail Path: {}",
                self.photo.thumbnail_path().unwrap().display()
            ));
            ui.label(format!("Width: {}", self.photo.metadata.width));
            ui.label(format!("Height: {}", self.photo.metadata.height));
            ui.label(format!(
                "Rotated Width: {}",
                self.photo.metadata.rotated_width
            ));
            ui.label(format!(
                "Rotated Height: {}",
                self.photo.metadata.rotated_height
            ));
            ui.label(format!("Rotation: {:?}", self.photo.metadata.rotation));
            ui.label(format!("Camera: {:?}", self.photo.metadata.camera));
            ui.label(format!("Date Time: {:?}", self.photo.metadata.date_time));
            ui.label(format!("ISO: {:?}", self.photo.metadata.iso));
            ui.label(format!(
                "Shutter Speed: {:?}",
                self.photo.metadata.shutter_speed
            ));
            ui.label(format!("Aperture: {:?}", self.photo.metadata.aperture));
            ui.label(format!(
                "Focal Length: {:?}",
                self.photo.metadata.focal_length
            ));
        })
        .response
    }
}
