use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    photo_database::PhotoQuery,
    photo_manager::PhotoManager,
    widget::photo_filter::{PhotoFilter, PhotoFilterState},
};

use super::{Modal, ModalActionResponse};

/// Modal wrapper for the photo filter widget
#[derive(Debug, Clone)]
pub struct PhotoFilterModal {
    filter_state: PhotoFilterState,
    available_tags: Vec<String>,
    initial_query: Option<PhotoQuery>,
}

impl PhotoFilterModal {
    /// Create a new photo filter modal with default state
    pub fn new() -> Self {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        let available_tags = photo_manager.with_lock(|pm| pm.all_tags());

        Self {
            filter_state: PhotoFilterState::default(),
            available_tags,
            initial_query: None,
        }
    }

    /// Create a new photo filter modal with existing query
    pub fn with_query(query: PhotoQuery) -> Self {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        let available_tags = photo_manager.with_lock(|pm| pm.all_tags());

        let mut filter_state = PhotoFilterState::default();

        // Apply the existing query to the filter state
        if let Some(ratings) = &query.ratings {
            filter_state.enabled_ratings = ratings.iter().copied().collect();
        }

        if let Some(tags) = &query.tags {
            filter_state.selected_tags = tags.iter().cloned().collect();
        }

        filter_state.grouping = query.grouping;

        Self {
            filter_state,
            available_tags,
            initial_query: Some(query),
        }
    }

    /// Get the current filter query
    pub fn get_query(&self) -> PhotoQuery {
        self.filter_state.to_query()
    }

    /// Check if the filter has been modified from its initial state
    pub fn is_modified(&self) -> bool {
        if let Some(ref initial) = self.initial_query {
            &self.filter_state.to_query() != initial
        } else {
            self.filter_state.has_active_filters()
        }
    }
}

impl Default for PhotoFilterModal {
    fn default() -> Self {
        Self::new()
    }
}

impl Modal for PhotoFilterModal {
    fn title(&self) -> String {
        "Filter Photos".to_string()
    }

    fn body_ui(&mut self, ui: &mut egui::Ui) {
        let _response = PhotoFilter::new(&mut self.filter_state)
            .available_tags(&self.available_tags)
            .show_grouping(true)
            .show(ui);
    }

    fn actions_ui(&mut self, ui: &mut egui::Ui) -> ModalActionResponse {
        let mut response = ModalActionResponse::None;

        ui.horizontal(|ui| {
            // Left side: Clear All button (only if filters are active)
            if self.filter_state.has_active_filters() {
                let clear_button = egui::Button::new(
                    egui::RichText::new("🗑 Clear All")
                        .color(ui.style().visuals.warn_fg_color)
                );
                if ui.add(clear_button).clicked() {
                    self.filter_state.reset();
                }
            }

            // Right side: Cancel and Apply buttons
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Apply button (primary action)
                let apply_text = if self.is_modified() {
                    "✓ Apply Changes"
                } else {
                    "Apply"
                };

                let apply_button = egui::Button::new(
                    egui::RichText::new(apply_text).strong()
                ).fill(ui.style().visuals.selection.bg_fill);

                if ui.add(apply_button).clicked() {
                    response = ModalActionResponse::Confirm;
                }

                ui.add_space(8.0);

                // Cancel button (secondary action)
                if ui.button("Cancel").clicked() {
                    response = ModalActionResponse::Cancel;
                }
            });
        });

        response
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::photo_grouping::PhotoGrouping;
    use crate::photo::PhotoRating;

    #[test]
    fn test_photo_filter_modal_with_query() {
        let query = PhotoQuery {
            ratings: Some(vec![PhotoRating::Yes]),
            tags: Some(vec!["landscape".to_string(), "__untagged__".to_string()]),
            grouping: PhotoGrouping::Date,
        };

        let modal = PhotoFilterModal {
            filter_state: PhotoFilterState::default(),
            available_tags: vec!["landscape".to_string()],
            initial_query: Some(query.clone()),
        };

        // Test that the modal remembers the initial query
        assert_eq!(modal.initial_query, Some(query));
    }

    #[test]
    fn test_photo_filter_modal_get_query() {
        let mut modal = PhotoFilterModal {
            filter_state: PhotoFilterState::default(),
            available_tags: vec!["test".to_string()],
            initial_query: None,
        };

        // Modify the filter state
        modal.filter_state.enabled_ratings.clear();
        modal.filter_state.enabled_ratings.insert(PhotoRating::Yes);
        modal
            .filter_state
            .selected_tags
            .insert("landscape".to_string());

        let query = modal.get_query();
        assert_eq!(query.ratings.unwrap(), vec![PhotoRating::Yes]);

        let tags = query.tags.unwrap();
        assert!(tags.contains(&"landscape".to_string()));
    }

    #[test]
    fn test_photo_filter_modal_is_modified() {
        // Test with no initial query
        let mut modal = PhotoFilterModal {
            filter_state: PhotoFilterState::default(),
            available_tags: vec![],
            initial_query: None,
        };

        assert!(!modal.is_modified());

        // Modify the state
        modal.filter_state.selected_tags.insert("test".to_string());
        assert!(modal.is_modified());

        // Test with initial query
        let initial_query = PhotoQuery {
            ratings: Some(vec![PhotoRating::Yes]),
            tags: None,
            grouping: PhotoGrouping::Date,
        };

        let mut modal = PhotoFilterModal {
            filter_state: PhotoFilterState::default(),
            available_tags: vec![],
            initial_query: Some(initial_query.clone()),
        };

        // Should be modified because default state doesn't match initial query
        assert!(modal.is_modified());

        // Set state to match initial query
        modal.filter_state.enabled_ratings.clear();
        modal.filter_state.enabled_ratings.insert(PhotoRating::Yes);
        modal.filter_state.grouping = PhotoGrouping::Date;

        // Should not be modified now
        assert!(!modal.is_modified());
    }

    #[test]
    fn test_photo_filter_modal_default() {
        let modal = PhotoFilterModal::default();
        assert_eq!(modal.title(), "Filter Photos");
        assert!(!modal.is_modified());
        assert!(modal.initial_query.is_none());
    }

    #[test]
    fn test_photo_filter_modal_as_any_mut() {
        let mut modal = PhotoFilterModal::default();
        let _any_mut = modal.as_any_mut();
        // Test that the method exists and can be called
    }

    #[test]
    fn test_photo_filter_modal_query_conversion() {
        let mut modal = PhotoFilterModal::default();

        // Test various filter combinations
        modal.filter_state.enabled_ratings.clear();
        modal.filter_state.enabled_ratings.insert(PhotoRating::Yes);
        modal
            .filter_state
            .enabled_ratings
            .insert(PhotoRating::Maybe);

        modal
            .filter_state
            .selected_tags
            .insert("landscape".to_string());
        modal
            .filter_state
            .selected_tags
            .insert("portrait".to_string());
        modal.filter_state.grouping = PhotoGrouping::Rating;

        let query = modal.get_query();

        let ratings = query.ratings.unwrap();
        assert!(ratings.contains(&PhotoRating::Yes));
        assert!(ratings.contains(&PhotoRating::Maybe));
        assert!(!ratings.contains(&PhotoRating::No));

        let tags = query.tags.unwrap();
        assert!(tags.contains(&"landscape".to_string()));
        assert!(tags.contains(&"portrait".to_string()));

        assert_eq!(query.grouping, PhotoGrouping::Rating);
    }

    #[test]
    fn test_photo_filter_modal_edge_cases() {
        // Test with empty ratings
        let mut modal = PhotoFilterModal::default();
        modal.filter_state.enabled_ratings.clear();

        let query = modal.get_query();
        assert!(query.ratings.is_none());

        // Test with only untagged
        let query = modal.get_query();
        assert!(query.tags.is_none());

        // Test with no grouping
        modal.filter_state.grouping = PhotoGrouping::Date;
        let query = modal.get_query();
        assert_eq!(query.grouping, PhotoGrouping::Date);
    }
}
