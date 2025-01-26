use eframe::egui::{self, Rect, Sense, Ui, Vec2};
use egui::UiBuilder;

pub struct AutoCenter {
    id: egui::Id,
}

impl AutoCenter {
    pub fn new(id: impl std::hash::Hash) -> Self {
        Self {
            id: egui::Id::new(id),
        }
    }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> egui::InnerResponse<R> {
        // First get the available size
        let available_size = ui.available_size();

        // Check if we have stored the content size from a previous frame
        let content_size: Option<Vec2> = ui.memory_mut(|mem| mem.data.get_temp(self.id));

        if let Some(stored_size) = content_size {
            // If we have the stored size, create a centered rect and show the content
            let centered_rect = Rect::from_center_size(ui.max_rect().center(), stored_size);

            let (_rect, response) = ui.allocate_exact_size(stored_size, Sense::hover());

            // Ensure the content is centered in the allocated space
            let mut child_ui = ui.new_child(
                UiBuilder::new()
                    .max_rect(centered_rect)
                    .layout(*ui.layout()),
            );

            let inner_response = add_contents(&mut child_ui);

            egui::InnerResponse::new(inner_response, response)
        } else {
            // If we don't have the stored size, measure the content first
            let mut measure_ui = ui.new_child(
                UiBuilder::new()
                    .sizing_pass()
                    .invisible()
                    .max_rect(Rect::from_min_size(ui.max_rect().min, available_size)),
            );

            let inner_response = add_contents(&mut measure_ui);

            // Store the measured size for the next frame
            let content_size = measure_ui.min_rect().size();
            ui.memory_mut(|mem| mem.data.insert_temp(self.id, content_size));

            // Create a dummy response for this frame
            let response = ui.allocate_rect(ui.max_rect(), Sense::hover());

            egui::InnerResponse::new(inner_response, response)
        }
    }
}
