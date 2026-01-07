use egui::{Response, Ui, Widget};
use std::collections::HashSet;
use strum::IntoEnumIterator;

use crate::{model::photo_grouping::PhotoGrouping, photo::PhotoRating, photo_database::PhotoQuery};

use super::{
    segment_control::SegmentControl,
    tag_chips::{TagChips, TagChipsState},
};

/// State for the photo filter widget
#[derive(Debug, Clone, PartialEq)]
pub struct PhotoFilterState {
    pub enabled_ratings: HashSet<PhotoRating>,
    pub selected_tags: HashSet<String>,
    pub grouping: PhotoGrouping,
    pub tag_chips_state: TagChipsState,
}

impl Default for PhotoFilterState {
    fn default() -> Self {
        Self {
            enabled_ratings: PhotoRating::iter().collect(),
            selected_tags: HashSet::new(),
            grouping: PhotoGrouping::default(),
            tag_chips_state: TagChipsState::new(),
        }
    }
}

impl PhotoFilterState {
    pub fn _new() -> Self {
        Self::default()
    }

    /// Convert the filter state to a PhotoQuery
    pub fn to_query(&self) -> PhotoQuery {
        PhotoQuery {
            ratings: if self.enabled_ratings.is_empty()
                || self.enabled_ratings.len() == PhotoRating::iter().count()
            {
                None
            } else {
                Some(self.enabled_ratings.iter().copied().collect())
            },
            tags: if self.selected_tags.is_empty() {
                None
            } else {
                Some(self.selected_tags.iter().cloned().collect::<Vec<String>>())
            },
            grouping: self.grouping,
        }
    }

    /// Reset all filters to default state
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Check if any filters are active (different from default)
    pub fn has_active_filters(&self) -> bool {
        (self.enabled_ratings.len() != PhotoRating::iter().count()
            && !self.enabled_ratings.is_empty())
            || !self.selected_tags.is_empty()
    }
}

/// Response from the photo filter widget
pub struct PhotoFilterResponse {
    pub response: Response,
    #[allow(dead_code)]
    pub(crate) changed: bool,
}

impl PhotoFilterResponse {
    #[allow(dead_code)]
    pub fn changed(&self) -> bool {
        self.changed
    }
}

/// Photo filter widget
pub struct PhotoFilter<'a> {
    state: &'a mut PhotoFilterState,
    available_tags: Vec<String>,
    show_grouping: bool,
}

impl<'a> PhotoFilter<'a> {
    pub fn new(state: &'a mut PhotoFilterState) -> Self {
        Self {
            state,
            available_tags: Vec::new(),
            show_grouping: true,
        }
    }

    /// Set the available tags for filtering
    pub fn available_tags(mut self, tags: &[String]) -> Self {
        self.available_tags = tags.to_vec();
        self
    }

    /// Whether to show the grouping options
    pub fn show_grouping(mut self, show: bool) -> Self {
        self.show_grouping = show;
        self
    }

    /// Show the filter widget and return response
    pub fn show(self, ui: &mut Ui) -> PhotoFilterResponse {
        let mut changed = false;

        let response = ui
            .allocate_ui(ui.available_size(), |ui| {
                ui.style_mut().spacing.item_spacing = egui::vec2(8.0, 10.0);

                ui.vertical(|ui| {
                    // Rating filter section
                    ui.scope(|ui| {
                        ui.style_mut().visuals.widgets.noninteractive.bg_fill =
                            ui.style().visuals.extreme_bg_color;
                        ui.style_mut().visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);

                        egui::Frame::group(ui.style())
                            .inner_margin(egui::Margin::same(12))
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.heading("Rating Filter");
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new("Select which ratings to show")
                                            .small()
                                            .color(ui.style().visuals.weak_text_color())
                                    );
                                    ui.add_space(8.0);

                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing.x = 12.0;
                                        for rating in PhotoRating::iter() {
                                            let mut is_enabled = self.state.enabled_ratings.contains(&rating);
                                            let checkbox_response = ui.checkbox(&mut is_enabled,
                                                egui::RichText::new(format!("{:?}", rating)).size(14.0));

                                            if checkbox_response.changed() {
                                                if is_enabled {
                                                    self.state.enabled_ratings.insert(rating);
                                                } else {
                                                    self.state.enabled_ratings.remove(&rating);
                                                }
                                                changed = true;
                                            }
                                        }
                                    });
                                });
                            });
                    });

                    ui.add_space(4.0);

                    // Tag filter section
                    ui.scope(|ui| {
                        ui.style_mut().visuals.widgets.noninteractive.bg_fill =
                            ui.style().visuals.extreme_bg_color;
                        ui.style_mut().visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);

                        egui::Frame::group(ui.style())
                            .inner_margin(egui::Margin::same(12))
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.heading("Tag Filter");
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new("Select tags to filter by (all selected tags must match)")
                                            .small()
                                            .color(ui.style().visuals.weak_text_color())
                                    );
                                    ui.add_space(8.0);

                                    let tag_response = TagChips::new(
                                        &mut self.state.selected_tags,
                                        &mut self.state.tag_chips_state,
                                    )
                                    .available_tags(&self.available_tags)
                                    .show_input(false)
                                    .show(ui);

                                    if tag_response.changed() {
                                        changed = true;
                                    }

                                    if self.available_tags.is_empty() {
                                        ui.add_space(4.0);
                                        ui.colored_label(
                                            ui.style().visuals.warn_fg_color,
                                            egui::RichText::new("No tags available").italics()
                                        );
                                    }
                                });
                            });
                    });

                    // Grouping section
                    if self.show_grouping {
                        ui.add_space(4.0);

                        ui.scope(|ui| {
                            ui.style_mut().visuals.widgets.noninteractive.bg_fill =
                                ui.style().visuals.extreme_bg_color;
                            ui.style_mut().visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);

                            egui::Frame::group(ui.style())
                                .inner_margin(egui::Margin::same(12))
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        ui.heading("Group By");
                                        ui.add_space(4.0);
                                        ui.label(
                                            egui::RichText::new("Organize photos by")
                                                .small()
                                                .color(ui.style().visuals.weak_text_color())
                                        );
                                        ui.add_space(8.0);

                                        let grouping_options = vec![
                                            (PhotoGrouping::Date, "Date".to_string()),
                                            (PhotoGrouping::Rating, "Rating".to_string()),
                                            (PhotoGrouping::Tag, "Tag".to_string()),
                                        ];

                                        let mut current_grouping = self.state.grouping;
                                        let segment_response =
                                            SegmentControl::new(&grouping_options, &mut current_grouping)
                                                .ui(ui);

                                        if segment_response.changed() {
                                            self.state.grouping = current_grouping;
                                            changed = true;
                                        }
                                    });
                                });
                        });
                    }

                    ui.add_space(8.0);

                    // Status indicator
                    ui.separator();
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        let filter_count = self.count_active_filters();
                        let status_text = if filter_count > 0 {
                            egui::RichText::new(format!("✓ {} filter(s) active", filter_count))
                                .color(ui.style().visuals.selection.bg_fill)
                                .strong()
                        } else {
                            egui::RichText::new("○ No filters active")
                                .color(ui.style().visuals.weak_text_color())
                        };
                        ui.label(status_text);
                    });
                });
            })
            .response;

        PhotoFilterResponse { response, changed }
    }

    fn count_active_filters(&self) -> usize {
        let mut count = 0;

        if self.state.enabled_ratings.len() != PhotoRating::iter().count() {
            count += 1;
        }

        if !self.state.selected_tags.is_empty() {
            count += 1;
        }
        count
    }
}

impl<'a> Widget for PhotoFilter<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.show(ui).response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;

    #[test]
    fn test_default_filter_state() {
        let state = PhotoFilterState::default();
        assert_eq!(state.enabled_ratings.len(), PhotoRating::iter().count());
        assert!(state.selected_tags.is_empty());
        assert_eq!(state.grouping, PhotoGrouping::default());
    }

    #[test]
    fn test_filter_state_to_query() {
        let mut state = PhotoFilterState::default();

        // Test with no filters
        let query = state.to_query();
        assert!(query.ratings.is_none());
        assert!(query.tags.is_none());

        // Test with specific filters
        state.enabled_ratings.clear();
        state.enabled_ratings.insert(PhotoRating::Yes);
        state.selected_tags.insert("landscape".to_string());

        let query = state.to_query();
        let ratings = query.ratings.unwrap();
        assert_eq!(ratings.len(), 1);
        assert!(ratings.contains(&PhotoRating::Yes));

        let tags = query.tags.unwrap();
        assert_eq!(tags.len(), 1);
        assert!(tags.contains(&"landscape".to_string()));
    }

    #[test]
    fn test_filter_state_has_active_filters() {
        let mut state = PhotoFilterState::default();
        assert!(!state.has_active_filters());

        // Remove one rating
        state.enabled_ratings.remove(&PhotoRating::Maybe);
        assert!(state.has_active_filters());

        // Reset and add a tag
        state = PhotoFilterState::default();
        state.selected_tags.insert("test".to_string());
        assert!(state.has_active_filters());

        state = PhotoFilterState::default();
        assert!(!state.has_active_filters());
    }

    #[test]
    fn test_filter_state_reset() {
        let mut state = PhotoFilterState::default();
        state.enabled_ratings.clear();
        state.selected_tags.insert("test".to_string());
        state.grouping = PhotoGrouping::default();

        state.reset();

        let default_state = PhotoFilterState::default();
        assert_eq!(state, default_state);
    }

    #[test]
    fn test_photo_filter_widget_basic() {
        let mut state = PhotoFilterState::default();
        let available_tags = vec!["landscape".to_string(), "portrait".to_string()];

        let mut harness = Harness::new_ui(|ui| {
            let response = PhotoFilter::new(&mut state)
                .available_tags(&available_tags)
                .show_grouping(true)
                .show(ui);

            assert!(!response.changed());
        });

        harness.run();
    }

    #[test]
    fn test_photo_filter_widget_no_grouping() {
        let mut state = PhotoFilterState::default();

        let mut harness = Harness::new_ui(|ui| {
            let response = PhotoFilter::new(&mut state).show_grouping(false).show(ui);

            assert!(!response.changed());
        });

        harness.run();
    }

    #[test]
    fn test_photo_filter_widget_no_tags() {
        let mut state = PhotoFilterState::default();

        let mut harness = Harness::new_ui(|ui| {
            let response = PhotoFilter::new(&mut state).available_tags(&[]).show(ui);

            assert!(!response.changed());
        });

        harness.run();
    }

    #[test]
    fn test_photo_filter_widget_with_selected_tags() {
        let mut state = PhotoFilterState::default();
        state.selected_tags.insert("landscape".to_string());
        state.selected_tags.insert("portrait".to_string());

        let available_tags = vec![
            "landscape".to_string(),
            "portrait".to_string(),
            "sunset".to_string(),
        ];

        let mut harness = Harness::new_ui(|ui| {
            let response = PhotoFilter::new(&mut state)
                .available_tags(&available_tags)
                .show(ui);

            assert!(!response.changed());
        });

        harness.run();
    }

    #[test]
    fn test_photo_filter_widget_with_modified_ratings() {
        let mut state = PhotoFilterState::default();
        state.enabled_ratings.clear();
        state.enabled_ratings.insert(PhotoRating::Yes);

        let mut harness = Harness::new_ui(|ui| {
            let response = PhotoFilter::new(&mut state).show(ui);

            assert!(!response.changed());
        });

        harness.run();
    }

    #[test]
    fn test_photo_filter_count_active_filters() {
        let mut default_state = PhotoFilterState::default();
        let filter = PhotoFilter::new(&mut default_state);
        assert_eq!(filter.count_active_filters(), 0);

        let mut state = PhotoFilterState::default();
        state.enabled_ratings.remove(&PhotoRating::Maybe);
        let filter = PhotoFilter::new(&mut state);
        assert_eq!(filter.count_active_filters(), 1);

        state.selected_tags.insert("test".to_string());
        let filter = PhotoFilter::new(&mut state);
        assert_eq!(filter.count_active_filters(), 2);
    }

    fn _test_photo_filter_visual_snapshots() {
        // Comprehensive filter widget test with all features
        let mut state = PhotoFilterState::default();
        state.enabled_ratings.clear();
        state.enabled_ratings.insert(PhotoRating::Yes);
        state.selected_tags.insert("landscape".to_string());

        let available_tags = vec![
            "landscape".to_string(),
            "portrait".to_string(),
            "sunset".to_string(),
            "travel".to_string(),
        ];

        let mut harness = Harness::new_ui(|ui| {
            PhotoFilter::new(&mut state)
                .available_tags(&available_tags)
                .show_grouping(true)
                .show(ui);
        });
        harness.fit_contents();
        harness.snapshot("photo_filter_comprehensive");
    }
}
