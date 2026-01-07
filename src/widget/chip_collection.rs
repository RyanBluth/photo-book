use std::fmt::Display;

use super::chip::{chip_selectable, chip_selectable_closable};
use egui::{Response, Ui, Widget};

pub struct _ChipCollection<'a, T: ToString> {
    items: &'a [T],
    selected_item: Option<&'a T>,
    closable: bool,
    spacing: f32,
}

pub struct ChipCollectionResponse {
    pub _response: Response,
    pub clicked_item: Option<usize>,
    pub closed_item: Option<usize>,
}

impl ChipCollectionResponse {
    pub fn clicked_item(&self) -> Option<usize> {
        self.clicked_item
    }

    pub fn closed_item(&self) -> Option<usize> {
        self.closed_item
    }
}

impl<'a, T: PartialEq + ToString> _ChipCollection<'a, T> {
    pub fn _new(items: &'a [T]) -> Self {
        Self {
            items,
            selected_item: None,
            closable: false,
            spacing: 4.0,
        }
    }

    pub fn _selected(mut self, selected: Option<&'a T>) -> Self {
        self.selected_item = selected;
        self
    }

    pub fn _closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    pub fn _spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }
}

impl<'a, T: PartialEq + Display> Widget for _ChipCollection<'a, T> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = self.spacing;
            ui.spacing_mut().item_spacing.y = self.spacing;

            for (_index, item) in self.items.iter().enumerate() {
                let text = item.to_string();
                let is_selected = self.selected_item.map(|sel| sel == item).unwrap_or(false);

                let _chip_response = if self.closable {
                    chip_selectable_closable(ui, &text, is_selected)
                } else {
                    chip_selectable(ui, &text, is_selected)
                };
            }
        })
        .response
    }
}

pub fn chip_collection<T: PartialEq + ToString>(
    ui: &mut Ui,
    items: &[T],
    selected: Option<&T>,
    closable: bool,
    spacing: f32,
) -> ChipCollectionResponse {
    let mut clicked_item = None;
    let mut closed_item = None;

    let response = ui
        .horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = spacing;
            ui.spacing_mut().item_spacing.y = spacing;

            for (index, item) in items.iter().enumerate() {
                let is_selected = selected.map(|sel| sel == item).unwrap_or(false);

                let chip_response = if closable {
                    chip_selectable_closable(ui, &item.to_string(), is_selected)
                } else {
                    chip_selectable(ui, &item.to_string(), is_selected)
                };

                if chip_response.clicked() {
                    clicked_item = Some(index);
                }

                if chip_response.close_clicked() {
                    closed_item = Some(index);
                }
            }
        })
        .response;

    ChipCollectionResponse {
        _response: response,
        clicked_item,
        closed_item,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;

    #[test]
    fn test_chip_collection_basic() {
        let items = vec!["Item 1", "Item 2", "Item 3"];

        let mut harness = Harness::new_ui(|ui| {
            let response = chip_collection(ui, &items, None, false, 4.0);

            assert!(response.clicked_item().is_none());
            assert!(response.closed_item().is_none());
        });

        harness.run();
    }

    #[test]
    fn test_chip_collection_with_selection() {
        let items = vec!["Item 1", "Item 2", "Item 3"];
        let selected = Some(&items[1]);

        let mut harness = Harness::new_ui(|ui| {
            let response = chip_collection(ui, &items, selected, false, 4.0);

            assert!(response.clicked_item().is_none());
            assert!(response.closed_item().is_none());
        });

        harness.run();
    }

    #[test]
    fn test_chip_collection_closable() {
        let items = vec!["Item 1", "Item 2", "Item 3"];

        let mut harness = Harness::new_ui(|ui| {
            let response = chip_collection(ui, &items, None, true, 4.0);

            assert!(response.clicked_item().is_none());
            assert!(response.closed_item().is_none());
        });

        harness.run();
    }

    #[test]
    fn test_chip_collection_widget_direct() {
        let items = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let selected_item = Some(&items[0]);

        let mut harness = Harness::new_ui(|ui| {
            let collection = _ChipCollection::_new(&items)
                ._selected(selected_item)
                ._closable(true)
                ._spacing(6.0);
            let response = ui.add(collection);
            assert!(!response.clicked());
        });

        harness.run();
    }

    #[test]
    fn test_empty_chip_collection() {
        let items: Vec<&str> = vec![];

        let mut harness = Harness::new_ui(|ui| {
            let response = chip_collection(ui, &items, None, false, 4.0);

            assert!(response.clicked_item().is_none());
            assert!(response.closed_item().is_none());
        });

        harness.run();
    }

    #[test]
    fn test_collection_response_methods() {
        let items = vec!["A", "B", "C"];

        let mut harness = Harness::new_ui(|ui| {
            let response = chip_collection(ui, &items, None, true, 4.0);

            // Test that methods exist and return expected types
            let _clicked_item: Option<usize> = response.clicked_item();
            let _closed_item: Option<usize> = response.closed_item();
            let _underlying_response: &Response = &response._response;
        });

        harness.run();
    }

    #[test]
    fn test_chip_collection_visual_snapshots() {
        // Basic chip collection
        let items = vec!["Photo", "Travel", "Nature"];
        let mut harness = Harness::new_ui(|ui| {
            chip_collection(ui, &items, None, false, 4.0);
        });
        harness.fit_contents();
        harness.snapshot("chip_collection_basic");

        // Chip collection with selection
        let items = vec!["All", "Recent", "Favorites", "Shared"];
        let selected = Some(&items[1]);
        let mut harness = Harness::new_ui(|ui| {
            chip_collection(ui, &items, selected, false, 4.0);
        });
        harness.fit_contents();
        harness.snapshot("chip_collection_selected");

        // Chip collection with close buttons
        let items = vec!["Tag1", "Tag2", "Tag3", "Tag4"];
        let mut harness = Harness::new_ui(|ui| {
            chip_collection(ui, &items, None, true, 4.0);
        });
        harness.fit_contents();
        harness.snapshot("chip_collection_closable");

        // Chip collection with both selection and close buttons
        let items = vec!["Photography", "Travel", "Nature", "Portrait"];
        let selected = Some(&items[0]);
        let mut harness = Harness::new_ui(|ui| {
            chip_collection(ui, &items, selected, true, 6.0);
        });
        harness.fit_contents();
        harness.snapshot("chip_collection_selected_closable");

        // Large collection that wraps
        let items = vec![
            "Very Long Tag Name",
            "Short",
            "Medium Length",
            "Another Long Tag Name",
            "Photography",
            "Travel",
            "Nature",
            "Portrait",
            "Landscape",
            "Street",
            "Macro",
            "Wildlife",
            "Architecture",
            "Documentary",
        ];
        let mut harness = Harness::new_ui(|ui| {
            ui.set_max_width(300.0);
            chip_collection(ui, &items, Some(&items[2]), false, 4.0);
        });
        harness.fit_contents();
        harness.snapshot("chip_collection_wrapping");

        // Empty collection
        let items: Vec<&str> = vec![];
        let mut harness = Harness::new_ui(|ui| {
            chip_collection(ui, &items, None, false, 4.0);
        });
        harness.fit_contents();
        harness.snapshot("chip_collection_empty");

        // Single item
        let items = vec!["Single"];
        let mut harness = Harness::new_ui(|ui| {
            chip_collection(ui, &items, Some(&items[0]), true, 4.0);
        });
        harness.fit_contents();
        harness.snapshot("chip_collection_single");
    }
}
