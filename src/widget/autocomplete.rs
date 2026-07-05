#![allow(dead_code)]

use std::{fmt::Debug, hash::Hash};

use egui::{Id, Key, Modifiers, Popup, Response, ScrollArea, TextEdit, Ui, Widget, WidgetText};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutocompleteState {
    open: bool,
    highlighted_index: Option<usize>,
}

impl AutocompleteState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.highlighted_index = None;
    }

    pub fn highlighted_index(&self) -> Option<usize> {
        self.highlighted_index
    }
}

pub struct AutocompleteResponse {
    pub response: Response,
    pub text_changed: bool,
    pub selected_text: Option<String>,
    pub submitted_text: Option<String>,
}

impl AutocompleteResponse {
    pub fn changed(&self) -> bool {
        self.text_changed
    }

    pub fn selected_text(&self) -> Option<&str> {
        self.selected_text.as_deref()
    }

    pub fn submitted_text(&self) -> Option<&str> {
        self.submitted_text.as_deref()
    }
}

pub struct Autocomplete<'a, S: AsRef<str>> {
    id: Id,
    text: &'a mut String,
    suggestions: &'a [S],
    state: &'a mut AutocompleteState,
    hint_text: Option<WidgetText>,
    desired_width: Option<f32>,
    max_popup_height: f32,
}

impl<'a, S: AsRef<str>> Autocomplete<'a, S> {
    pub fn new(
        id_salt: impl Hash + Debug,
        text: &'a mut String,
        suggestions: &'a [S],
        state: &'a mut AutocompleteState,
    ) -> Self {
        Self {
            id: Id::new(id_salt),
            text,
            suggestions,
            state,
            hint_text: None,
            desired_width: None,
            max_popup_height: 180.0,
        }
    }

    pub fn id(mut self, id: Id) -> Self {
        self.id = id;
        self
    }

    pub fn hint_text(mut self, hint_text: impl Into<WidgetText>) -> Self {
        self.hint_text = Some(hint_text.into());
        self
    }

    pub fn desired_width(mut self, desired_width: f32) -> Self {
        self.desired_width = Some(desired_width);
        self
    }

    pub fn max_popup_height(mut self, max_popup_height: f32) -> Self {
        self.max_popup_height = max_popup_height;
        self
    }

    pub fn show(self, ui: &mut Ui) -> AutocompleteResponse {
        let popup_id = self.id.with("popup");
        let mut text_edit = TextEdit::singleline(self.text).id(self.id);

        if let Some(hint_text) = self.hint_text {
            text_edit = text_edit.hint_text(hint_text);
        }

        if let Some(desired_width) = self.desired_width {
            text_edit = text_edit.desired_width(desired_width);
        }

        let mut response = text_edit.show(ui).response.response;
        let mut text_changed = response.changed();
        let mut selected_text = None;
        let mut submitted_text = None;

        if response.gained_focus() || response.clicked() || text_changed {
            self.state.open();
        }

        let mut filtered_indices = filtered_suggestion_indices(self.text, self.suggestions);
        clamp_highlight(self.state, filtered_indices.len());

        if response.has_focus() || response.lost_focus() {
            handle_keyboard(
                ui,
                self.text,
                self.suggestions,
                self.state,
                &filtered_indices,
                &mut response,
                &mut text_changed,
                &mut selected_text,
                &mut submitted_text,
            );

            filtered_indices = filtered_suggestion_indices(self.text, self.suggestions);
            clamp_highlight(self.state, filtered_indices.len());
        }

        let popup_open = self.state.open && !filtered_indices.is_empty();
        let mut clicked_suggestion = None;
        let mut clicked_outside = false;

        if popup_open {
            let highlighted_index = self.state.highlighted_index;

            let popup_response = Popup::from_response(&response)
                .id(popup_id)
                .width(response.rect.width())
                .open(true)
                .show(|ui| {
                    ui.set_min_width(response.rect.width());

                    ScrollArea::vertical()
                        .max_height(self.max_popup_height)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for (filtered_index, suggestion_index) in
                                filtered_indices.iter().copied().enumerate()
                            {
                                let suggestion = self.suggestions[suggestion_index].as_ref();
                                let selected = highlighted_index == Some(filtered_index);

                                if ui.selectable_label(selected, suggestion).clicked() {
                                    clicked_suggestion = Some(suggestion.to_owned());
                                    ui.close();
                                }
                            }
                        });
                });

            clicked_outside = popup_response
                .as_ref()
                .map(|inner| inner.response.clicked_elsewhere() && !response.clicked())
                .unwrap_or(false);
        }

        if let Some(suggestion) = clicked_suggestion {
            accept_text(
                self.text,
                suggestion,
                &mut response,
                &mut text_changed,
                &mut selected_text,
            );
            self.state.close();
        } else if clicked_outside {
            self.state.close();
        } else if response.has_focus() && !filtered_indices.is_empty() {
            self.state.open();
        } else if filtered_indices.is_empty() {
            self.state.close();
        }

        AutocompleteResponse {
            response,
            text_changed,
            selected_text,
            submitted_text,
        }
    }
}

impl<S: AsRef<str>> Widget for Autocomplete<'_, S> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.show(ui).response
    }
}

fn filtered_suggestion_indices<S: AsRef<str>>(text: &str, suggestions: &[S]) -> Vec<usize> {
    let query = text.to_lowercase();

    suggestions
        .iter()
        .enumerate()
        .filter_map(|(index, suggestion)| {
            let suggestion = suggestion.as_ref();

            (query.is_empty() || suggestion.to_lowercase().contains(&query)).then_some(index)
        })
        .collect()
}

fn clamp_highlight(state: &mut AutocompleteState, filtered_len: usize) {
    if filtered_len == 0
        || state
            .highlighted_index
            .is_some_and(|index| index >= filtered_len)
    {
        state.highlighted_index = None;
    }
}

fn handle_keyboard<S: AsRef<str>>(
    ui: &mut Ui,
    text: &mut String,
    suggestions: &[S],
    state: &mut AutocompleteState,
    filtered_indices: &[usize],
    response: &mut Response,
    text_changed: &mut bool,
    selected_text: &mut Option<String>,
    submitted_text: &mut Option<String>,
) {
    let escape = ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
    if escape {
        state.close();
        return;
    }

    let arrow_down = ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowDown));
    if arrow_down && !filtered_indices.is_empty() {
        state.open();
        state.highlighted_index = Some(match state.highlighted_index {
            Some(index) => (index + 1).min(filtered_indices.len() - 1),
            None => 0,
        });
    }

    let arrow_up = ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowUp));
    if arrow_up && !filtered_indices.is_empty() {
        state.open();
        state.highlighted_index = Some(match state.highlighted_index {
            Some(index) => index.saturating_sub(1),
            None => filtered_indices.len() - 1,
        });
    }

    let enter = ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter));
    if !enter {
        return;
    }

    let submitted = state
        .highlighted_index
        .and_then(|highlighted_index| filtered_indices.get(highlighted_index).copied())
        .map(|suggestion_index| suggestions[suggestion_index].as_ref().to_owned());

    if let Some(suggestion) = submitted {
        accept_text(
            text,
            suggestion.clone(),
            response,
            text_changed,
            selected_text,
        );
        *submitted_text = Some(suggestion);
    } else {
        *submitted_text = Some(text.clone());
    }

    state.close();
}

fn accept_text(
    text: &mut String,
    accepted_text: String,
    response: &mut Response,
    text_changed: &mut bool,
    selected_text: &mut Option<String>,
) {
    if *text != accepted_text {
        *text = accepted_text.clone();
        *text_changed = true;
        response.mark_changed();
    }

    *selected_text = Some(accepted_text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_matches_all_suggestions() {
        let suggestions = ["Family", "Travel", "Portrait"];

        assert_eq!(filtered_suggestion_indices("", &suggestions), vec![0, 1, 2]);
    }

    #[test]
    fn filtering_is_case_insensitive() {
        let suggestions = ["Family", "Travel", "Portrait"];

        assert_eq!(filtered_suggestion_indices("TR", &suggestions), vec![1, 2]);
    }
}
