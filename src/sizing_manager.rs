use std::collections::HashMap;

use egui::{Id, Layout, Rect, Ui, UiBuilder, Vec2};

/// Sizing data stored for a particular widget id.
#[derive(Debug, Clone, Copy)]
struct SizingEntry {
    available_size: Vec2,
    measured_size: Vec2,
}

/// Central manager for widget sizing state.
#[derive(Debug, Clone, Default)]
pub struct SizingManager {
    entries: HashMap<Id, SizingEntry>,
}

#[allow(dead_code)]
impl SizingManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the cached size for `id`, if it has already been measured.
    pub fn cached_size(&self, id: Id) -> Option<Vec2> {
        self.entries.get(&id).map(|entry| entry.measured_size)
    }

    /// Return the cached size for `id` only if it matches the current
    /// `available_rect_before_wrap` size.
    pub fn cached_size_for_current_rect(&self, ui: &Ui, id: Id) -> Option<Vec2> {
        self.cached_size_for_rect(id, ui.available_rect_before_wrap())
    }

    /// Return the cached size for `id` only if it matches `available_rect`.
    pub fn cached_size_for_rect(&self, id: Id, available_rect: Rect) -> Option<Vec2> {
        let entry = self.entries.get(&id)?;
        sizes_match(entry.available_size, available_rect.size()).then_some(entry.measured_size)
    }

    /// Measure `add_contents` when the current available rect size changed,
    /// otherwise return the cached measured size for `id`.
    pub fn size(&mut self, ui: &mut Ui, id: Id, add_contents: impl FnOnce(&mut Ui)) -> Vec2 {
        let available_rect = ui.available_rect_before_wrap();
        self.size_in_rect(ui, id, available_rect, *ui.layout(), add_contents)
    }

    pub fn sized<'a>(
        &mut self,
        ui: &mut Ui,
        id: Id,
        mut add_contents: impl FnMut(&mut Ui) + 'a,
        layout: impl FnOnce(&mut Ui, Vec2, Box<dyn FnOnce(&mut Ui) + 'a>),
    ) {
        let available_rect = ui.available_rect_before_wrap();
        let available_size = available_rect.size();

        if let Some(entry) = self.entries.get(&id) {
            if sizes_match(entry.available_size, available_size) {
                layout(ui, entry.measured_size, Box::new(add_contents));
                return;
            }
        }

        let mut measure_ui = ui.new_child(
            UiBuilder::new()
                .id(id.with("sizing_pass"))
                .max_rect(available_rect)
                .layout(*ui.layout())
                .sizing_pass()
                .invisible(),
        );

        add_contents(&mut measure_ui);

        let measured_size = measure_ui.min_rect().size();
        self.entries.insert(
            id,
            SizingEntry {
                available_size,
                measured_size,
            },
        );

        layout(ui, measured_size, Box::new(add_contents));
    }

    /// Measure `add_contents` inside `available_rect` when that rect's size
    /// changed, otherwise return the cached measured size for `id`.
    pub fn size_in_rect(
        &mut self,
        ui: &mut Ui,
        id: Id,
        available_rect: Rect,
        layout: Layout,
        add_contents: impl FnOnce(&mut Ui),
    ) -> Vec2 {
        let available_size = available_rect.size();

        if let Some(entry) = self.entries.get(&id) {
            if sizes_match(entry.available_size, available_size) {
                return entry.measured_size;
            }
        }

        let mut measure_ui = ui.new_child(
            UiBuilder::new()
                .id(id.with("sizing_pass"))
                .max_rect(available_rect)
                .layout(layout)
                .sizing_pass()
                .invisible(),
        );

        add_contents(&mut measure_ui);

        let measured_size = measure_ui.min_rect().size();
        self.entries.insert(
            id,
            SizingEntry {
                available_size,
                measured_size,
            },
        );

        measured_size
    }

    /// Remove cached sizing data for a widget.
    pub fn remove(&mut self, id: Id) {
        self.entries.remove(&id);
    }

    /// Remove all cached sizing data.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

fn sizes_match(a: Vec2, b: Vec2) -> bool {
    (a.x - b.x).abs() <= 1.0 && (a.y - b.y).abs() <= 1.0
}
