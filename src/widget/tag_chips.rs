use std::collections::HashSet;
use egui::{Key, Response, Ui};

use crate::model::editable_value::EditableValue;
use super::chip_collection::chip_collection;

/// State for the tag chips widget
#[derive(Debug, Clone, PartialEq)]
pub struct TagChipsState {
    pub tag_input: EditableValue<String>,
}

impl TagChipsState {
    pub fn new() -> Self {
        Self {
            tag_input: EditableValue::new(String::new()),
        }
    }
}

impl Default for TagChipsState {
    fn default() -> Self {
        Self::new()
    }
}

/// Response from the tag chips widget
pub struct TagChipsResponse {
    pub response: Response,
    pub changed: bool,
}

impl TagChipsResponse {
    pub fn changed(&self) -> bool {
        self.changed
    }
}



/// Tag chips widget for managing tags with a chip-based interface
pub struct TagChips<'a> {
    selected_tags: &'a mut HashSet<String>,
    state: &'a mut TagChipsState,
    available_tags: Option<&'a [String]>,
    show_input: bool,
    label: Option<String>,
    spacing: f32,
}

impl<'a> TagChips<'a> {
    /// Create a new tag chips widget
    pub fn new(selected_tags: &'a mut HashSet<String>, state: &'a mut TagChipsState) -> Self {
        Self {
            selected_tags,
            state,
            available_tags: None,
            show_input: true,
            label: None,
            spacing: 4.0,
        }
    }

    pub fn available_tags(mut self, available_tags: &'a [String]) -> Self {
        self.available_tags = Some(available_tags);
        self
    }

    /// Whether to show the text input for adding new tags
    pub fn show_input(mut self, show_input: bool) -> Self {
        self.show_input = show_input;
        self
    }

    /// Set a label for the widget
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set spacing between chips
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    /// Show the tag chips widget
    pub fn show(self, ui: &mut Ui) -> TagChipsResponse {
        let mut changed = false;

        let response = ui.vertical(|ui| {
            // Show label if provided
            if let Some(label) = &self.label {
                ui.label(label);
                ui.add_space(4.0);
            }

            // Show available tags as selectable chips (excluding already selected ones)
            if let Some(available_tags) = self.available_tags {
                if !available_tags.is_empty() {
                    ui.label("Available tags:");
                    ui.add_space(4.0);

                    let unselected_tags: Vec<String> = available_tags
                        .iter()
                        .filter(|tag| !self.selected_tags.contains(*tag))
                        .cloned()
                        .collect();

                    if !unselected_tags.is_empty() {
                        let chip_response = chip_collection(
                            ui,
                            &unselected_tags,
                            None,
                            false, // not closable
                            self.spacing,
                        );

                        // Handle tag selection
                        if let Some(clicked_idx) = chip_response.clicked_item() {
                            if let Some(tag) = unselected_tags.get(clicked_idx) {
                                self.selected_tags.insert(tag.clone());
                                changed = true;
                            }
                        }
                    } else {
                        ui.label("All available tags are selected");
                    }

                    // Show selected tags as closable chips
                    if !self.selected_tags.is_empty() {
                        ui.add_space(8.0);
                        ui.label("Selected tags:");
                        ui.add_space(4.0);

                        let selected_vec: Vec<String> = self.selected_tags.iter().cloned().collect();
                        let selected_chip_response = chip_collection(
                            ui,
                            &selected_vec,
                            None,
                            true, // closable
                            self.spacing,
                        );

                        // Handle tag removal
                        if let Some(removed_idx) = selected_chip_response.closed_item() {
                            if let Some(tag) = selected_vec.get(removed_idx) {
                                self.selected_tags.remove(tag);
                                changed = true;
                            }
                        }
                    }
                } else {
                    ui.label("No tags available");
                }
            }

            // Show input for adding new tags (only if enabled)
            if self.show_input {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Add tag:");

                    let response = ui.text_edit_singleline(self.state.tag_input.editable_value());

                    if response.gained_focus() {
                        self.state.tag_input.begin_editing();
                    }

                    let should_add_tag = ui.button("Add").clicked() ||
                        (response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));

                    if should_add_tag {
                        self.state.tag_input.end_editing();
                        let tag = self.state.tag_input.value().trim().to_string();
                        if !tag.is_empty() && !self.selected_tags.contains(&tag) {
                            self.selected_tags.insert(tag);
                            self.state.tag_input = EditableValue::new(String::new());
                            changed = true;
                        }
                    }
                });
            }
        }).response;

        TagChipsResponse {
            response,
            changed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;

    #[test]
    fn test_tag_chips_with_input() {
        let mut selected_tags = HashSet::new();
        selected_tags.insert("existing".to_string());
        let mut state = TagChipsState::new();

        let mut harness = Harness::new_ui(|ui| {
            let response = TagChips::new(&mut selected_tags, &mut state)
                .label("Tags")
                .show_input(true)
                .show(ui);

            assert!(!response.changed());
        });

        harness.run();
    }

    #[test]
    fn test_tag_chips_without_input() {
        let mut selected_tags = HashSet::new();
        let mut state = TagChipsState::new();
        let available_tags = vec!["tag1".to_string(), "tag2".to_string()];

        let mut harness = Harness::new_ui(|ui| {
            let response = TagChips::new(&mut selected_tags, &mut state)
                .available_tags(&available_tags)
                .show_input(false)
                .show(ui);

            assert!(!response.changed());
        });

        harness.run();
    }

    #[test]
    fn test_tag_chips_state_default() {
        let state = TagChipsState::default();
        assert_eq!(state.tag_input.value(), "");
    }

    #[test]
    fn test_tag_chips_builder_pattern() {
        let mut selected_tags = HashSet::new();
        let mut state = TagChipsState::new();
        let available_tags = vec!["available".to_string()];

        let mut harness = Harness::new_ui(|ui| {
            let response = TagChips::new(&mut selected_tags, &mut state)
                .available_tags(&available_tags)
                .show_input(true)
                .label("Test Tags")
                .spacing(6.0)
                .show(ui);

            assert!(!response.changed());
        });

        harness.run();
    }

    #[test]
    fn test_tag_chips_visual_snapshots() {
        // With input and existing tags
        let mut selected_tags = HashSet::new();
        selected_tags.insert("Photography".to_string());
        selected_tags.insert("Travel".to_string());
        let mut state = TagChipsState::new();
        let available_tags = vec!["Nature".to_string(), "Portrait".to_string()];

        let mut harness = Harness::new_ui(|ui| {
            TagChips::new(&mut selected_tags, &mut state)
                .available_tags(&available_tags)
                .label("Tags")
                .show_input(true)
                .show(ui);
        });
        harness.fit_contents();
        harness.snapshot("tag_chips_with_input");

        // Without input (filter mode)
        let mut selected_tags = HashSet::new();
        selected_tags.insert("Travel".to_string());
        let mut state = TagChipsState::new();
        let available_tags = vec![
            "Photography".to_string(),
            "Travel".to_string(),
            "Nature".to_string(),
            "Portrait".to_string()
        ];

        let mut harness = Harness::new_ui(|ui| {
            TagChips::new(&mut selected_tags, &mut state)
                .available_tags(&available_tags)
                .show_input(false)
                .show(ui);
        });
        harness.fit_contents();
        harness.snapshot("tag_chips_without_input");

        // Empty state
        let mut selected_tags = HashSet::new();
        let mut state = TagChipsState::new();

        let mut harness = Harness::new_ui(|ui| {
            TagChips::new(&mut selected_tags, &mut state)
                .show_input(true)
                .show(ui);
        });
        harness.fit_contents();
        harness.snapshot("tag_chips_empty");
    }
}
