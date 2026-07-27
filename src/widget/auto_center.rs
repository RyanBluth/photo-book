use eframe::egui::{self, Rect, Sense, Ui};
use egui::UiBuilder;

pub struct AutoCenter {
    id: egui::Id,
}

impl AutoCenter {
    pub fn new(id: impl std::hash::Hash + std::fmt::Debug) -> Self {
        Self {
            id: egui::Id::new(id),
        }
    }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        mut add_contents: impl FnMut(&mut Ui) -> R,
    ) -> egui::InnerResponse<R> {
        let available_rect = ui.available_rect_before_wrap();
        let mut measure_ui = ui.new_child(
            UiBuilder::new()
                .id_salt(self.id.with("measure"))
                .sizing_pass()
                .invisible()
                .max_rect(available_rect),
        );
        let _ = add_contents(&mut measure_ui);

        let content_size = measure_ui.min_rect().size().min(available_rect.size());
        let centered_rect = Rect::from_center_size(available_rect.center(), content_size);
        let response = ui.allocate_rect(centered_rect, Sense::hover());
        let inner = ui
            .scope_builder(
                UiBuilder::new()
                    .id_salt(self.id.with("content"))
                    .max_rect(centered_rect)
                    .layout(*ui.layout()),
                |ui| add_contents(ui),
            )
            .inner;

        egui::InnerResponse::new(inner, response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;

    #[test]
    fn first_pass_renders_and_returns_the_visible_geometry() {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(400.0, 200.0))
            .build_ui(|ui| {
                let result = AutoCenter::new("test-auto-center").show(ui, |ui| ui.button("Apply"));
                assert!(result.response.rect.contains_rect(result.inner.rect));
                assert!((result.response.rect.center().x - ui.max_rect().center().x).abs() < 1.0);
            });

        harness.run();
    }
}
