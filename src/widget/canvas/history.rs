use std::{collections::HashSet, fmt::Display};

use egui::Id;
use indexmap::IndexMap;

use crate::{
    history::{HistoricallyEqual, UndoRedoStack},
    id::LayerId,
    layout::LayoutNode,
    model::edit_state::EditablePage,
    widget::{canvas_info::layers::Layer, edit_response::EditResponse},
};

use super::{CanvasState, MultiSelect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasHistoryKind {
    Transform,
    AddPhoto,
    Paste,
    Crop,
    DeleteLayers,
    AddText,
    EditText,
    AdjustPhoto,
    QuickLayout,
    AddShape,
    Arrange,
    EditProperties,
    EditLayers,
}

impl Display for CanvasHistoryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transform => write!(f, "Transform"),
            Self::AddPhoto => write!(f, "Add Photo"),
            Self::Paste => write!(f, "Paste Layers"),
            Self::Crop => write!(f, "Crop Photo"),
            Self::DeleteLayers => write!(f, "Delete Layers"),
            Self::AddText => write!(f, "Add Text"),
            Self::EditText => write!(f, "Edit Text"),
            Self::AdjustPhoto => write!(f, "Adjust Photo"),
            Self::QuickLayout => write!(f, "Quick Layout"),
            Self::AddShape => write!(f, "Add Shape"),
            Self::Arrange => write!(f, "Arrange Layers"),
            Self::EditProperties => write!(f, "Edit Properties"),
            Self::EditLayers => write!(f, "Edit Layers"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CanvasHistory {
    layers: IndexMap<LayerId, Layer>,
    /// Auxiliary group geometry for restoring a multiselect transform. It is
    /// not itself a document change; see `HistoricallyEqual` below.
    multi_select: Option<MultiSelect>,
    page: EditablePage,
    quick_layout_order: Vec<LayerId>,
    last_quick_layout: Option<LayoutNode>,
}

impl CanvasHistory {
    fn capture(state: &CanvasState) -> Self {
        let layers = state
            .layers
            .iter()
            .map(|(id, layer)| (*id, layer.persistent_clone()))
            .collect();

        let mut multi_select = state.multi_select.clone();
        if let Some(multi_select) = &mut multi_select {
            multi_select.transformable_state = multi_select.transformable_state.persistent_clone();
            for child in &mut multi_select.selected_layers {
                child.transformable_state = child.transformable_state.persistent_clone();
            }
        }

        Self {
            layers,
            multi_select,
            page: EditablePage::new(state.page.value.clone()),
            quick_layout_order: state.quick_layout_order.clone(),
            last_quick_layout: state.last_quick_layout.clone(),
        }
    }

    fn apply_to(self, state: &mut CanvasState) {
        let current_layer_handle_modes = state
            .layers
            .iter()
            .map(|(id, layer)| (*id, layer.transform_state.handle_mode))
            .collect::<IndexMap<_, _>>();
        let current_handle_mode = state
            .multi_select
            .as_ref()
            .map(|multi_select| multi_select.transformable_state.handle_mode);
        let mut selected_layer_ids = state
            .layers
            .iter()
            .filter_map(|(id, layer)| layer.selected.then_some(*id))
            .collect::<HashSet<_>>();

        state.layers = self.layers;
        selected_layer_ids.retain(|id| state.layers.contains_key(id));
        for (id, layer) in &mut state.layers {
            layer.selected = selected_layer_ids.contains(id);
            if let Some(handle_mode) = current_layer_handle_modes.get(id) {
                layer.transform_state.handle_mode = *handle_mode;
            }
        }

        let multi_select_layer_ids = selected_layer_ids
            .iter()
            .copied()
            .filter(|id| {
                state
                    .layers
                    .get(id)
                    .is_some_and(CanvasState::is_layer_canvas_selectable)
            })
            .collect::<HashSet<_>>();

        let snapshot_matches_selection = self.multi_select.as_ref().is_some_and(|multi_select| {
            multi_select.selected_layers.len() == multi_select_layer_ids.len()
                && multi_select
                    .selected_layers
                    .iter()
                    .all(|child| multi_select_layer_ids.contains(&child.id))
                && multi_select.matches_layers(&state.layers)
        });
        state.multi_select = if snapshot_matches_selection {
            self.multi_select
        } else if multi_select_layer_ids.len() > 1 {
            Some(MultiSelect::new(&state.layers))
        } else {
            None
        };
        if let (Some(handle_mode), Some(multi_select)) =
            (current_handle_mode, &mut state.multi_select)
        {
            multi_select.transformable_state.handle_mode = handle_mode;
        }

        state.page = self.page;
        state.quick_layout_order = self.quick_layout_order;
        state.last_quick_layout = self.last_quick_layout;
    }
}

impl HistoricallyEqual for CanvasHistory {
    fn historically_equal_to(&self, other: &Self) -> bool {
        // Selection and its cached MultiSelect geometry are UI state. The cache
        // is carried in snapshots only so group geometry can be restored when
        // it still matches the current selection.
        self.layers.as_slice() == other.layers.as_slice()
            && self.page == other.page
            && self.quick_layout_order == other.quick_layout_order
            && self.last_quick_layout == other.last_quick_layout
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasHistoryManager {
    stack: UndoRedoStack<CanvasHistoryKind, CanvasHistory>,
    pending_edit: Option<PendingCanvasEdit>,
}

#[derive(Debug, Clone, PartialEq)]
struct PendingCanvasEdit {
    kind: CanvasHistoryKind,
    owner: Option<Id>,
    snapshot: CanvasHistory,
}

impl CanvasHistoryManager {
    pub fn preview() -> Self {
        Self::with_initial_state(CanvasState::new())
    }

    pub fn with_initial_state(state: CanvasState) -> Self {
        Self {
            stack: UndoRedoStack::new(CanvasHistory::capture(&state)),
            pending_edit: None,
        }
    }

    pub fn undo(&mut self, state: &mut CanvasState) {
        self.finish_pending();
        self.stack.undo().apply_to(state);
    }

    pub fn redo(&mut self, state: &mut CanvasState) {
        self.finish_pending();
        self.stack.redo().apply_to(state);
    }

    /// Applies one complete document edit and records it when it changed the
    /// persistent canvas state.
    pub fn apply<T>(
        &mut self,
        kind: CanvasHistoryKind,
        state: &mut CanvasState,
        edit: impl FnOnce(&mut CanvasState) -> T,
    ) -> T {
        let before = CanvasHistory::capture(state);
        self.finish_pending();
        let result = edit(state);
        let after = CanvasHistory::capture(state);
        if !before.historically_equal_to(&after) {
            self.stack.save_history(kind, after);
        }
        result
    }

    /// Records the latest state of a continuous edit without pushing another
    /// undo entry. Switching edit kinds commits the previous pending snapshot.
    pub fn update(&mut self, kind: CanvasHistoryKind, state: &CanvasState) {
        self.update_owned(kind, None, state);
    }

    fn update_owned(&mut self, kind: CanvasHistoryKind, owner: Option<Id>, state: &CanvasState) {
        if self
            .pending_edit
            .as_ref()
            .is_some_and(|pending| pending.kind != kind || pending.owner != owner)
        {
            self.finish_pending();
        }
        self.pending_edit = Some(PendingCanvasEdit {
            kind,
            owner,
            snapshot: CanvasHistory::capture(state),
        });
    }

    /// Records a control response and commits it when that control no longer
    /// owns the interaction.
    pub fn record(&mut self, kind: CanvasHistoryKind, state: &CanvasState, response: EditResponse) {
        if self.pending_edit.as_ref().is_some_and(|pending| {
            pending.kind == kind && pending.owner.is_some() && pending.owner != response.owner
        }) {
            self.finish_pending();
        }
        if response.changed {
            self.update_owned(kind, response.owner, state);
        }
        if !response.active {
            self.finish(kind);
        }
    }

    pub fn finish(&mut self, kind: CanvasHistoryKind) {
        if self
            .pending_edit
            .as_ref()
            .is_some_and(|pending| pending.kind == kind)
        {
            self.finish_pending();
        }
    }

    /// Commits any continuous edit currently being collected.
    ///
    /// Call this when the UI scope that owns an edit is about to disappear,
    /// such as when changing pages or hiding an editor pane.
    pub fn finish_pending(&mut self) {
        if let Some(pending) = self.pending_edit.take() {
            self.stack.save_history(pending.kind, pending.snapshot);
        }
    }

    #[cfg(test)]
    pub(crate) fn history_len(&self) -> usize {
        self.stack.history.len()
    }

    #[cfg(test)]
    pub(crate) fn index(&self) -> usize {
        self.stack.index
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use egui::{Pos2, Rect, Vec2};
    use egui_kittest::{Harness, kittest::Queryable};

    use super::*;
    use crate::widget::{
        canvas::CanvasPhoto,
        canvas_info::{
            layers::{Layer, LayerContent},
            panel::CanvasProperties,
        },
        photo_adjustments::{PhotoAdjustmentsEditor, PhotoAdjustmentsState},
        transformable::{ResizeMode, TransformHandleMode},
    };
    use crate::{
        model::photo_adjustments::PhotoAdjustments,
        photo::{MetadataCollection, Photo, PhotoMetadata, PhotoMetadataField},
    };

    fn state_with_layer() -> (CanvasState, LayerId) {
        let mut state = CanvasState::new();
        let mut layer = Layer::new_rectangle_shape_layer();
        let layer_id = layer.id;
        layer.transform_state.rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 100.0));
        state.layers.insert(layer_id, layer);
        (state, layer_id)
    }

    fn move_layer_to(state: &mut CanvasState, layer_id: LayerId, x: f32) {
        let rect = &mut state
            .layers
            .get_mut(&layer_id)
            .unwrap()
            .transform_state
            .rect;
        *rect = rect.translate(Vec2::new(x - rect.left(), 0.0));
    }

    fn test_photo() -> Photo {
        let path = PathBuf::from("history-test.jpg");
        let mut fields = MetadataCollection::new();
        fields.insert(PhotoMetadataField::Path(path.clone()));
        fields.insert(PhotoMetadataField::Width(400));
        fields.insert(PhotoMetadataField::Height(200));
        fields.insert(PhotoMetadataField::RotatedWidth(400));
        fields.insert(PhotoMetadataField::RotatedHeight(200));
        Photo::with_metadata(path, PhotoMetadata { fields })
    }

    #[test]
    fn continuous_edits_coalesce_until_finished() {
        let (mut state, layer_id) = state_with_layer();
        let mut history = CanvasHistoryManager::with_initial_state(state.clone());

        move_layer_to(&mut state, layer_id, 10.0);
        history.update(CanvasHistoryKind::Transform, &state);
        move_layer_to(&mut state, layer_id, 20.0);
        history.update(CanvasHistoryKind::Transform, &state);
        history.finish(CanvasHistoryKind::Transform);

        move_layer_to(&mut state, layer_id, 30.0);
        history.update(CanvasHistoryKind::Transform, &state);
        history.finish(CanvasHistoryKind::Transform);

        assert_eq!(history.history_len(), 2);
        history.undo(&mut state);
        assert_eq!(state.layers[&layer_id].transform_state.rect.left(), 20.0);
        history.undo(&mut state);
        assert_eq!(state.layers[&layer_id].transform_state.rect.left(), 0.0);
    }

    #[test]
    fn switching_edit_kind_uses_the_previous_pending_snapshot() {
        let (mut state, layer_id) = state_with_layer();
        let mut history = CanvasHistoryManager::with_initial_state(state.clone());

        move_layer_to(&mut state, layer_id, 100.0);
        history.update(CanvasHistoryKind::EditProperties, &state);

        // The new edit is reported after its first mutation, as happens with an
        // immediate-mode transform widget.
        move_layer_to(&mut state, layer_id, 110.0);
        history.update(CanvasHistoryKind::Transform, &state);
        history.finish(CanvasHistoryKind::Transform);

        assert_eq!(history.history_len(), 2);
        history.undo(&mut state);
        assert_eq!(state.layers[&layer_id].transform_state.rect.left(), 100.0);
        history.undo(&mut state);
        assert_eq!(state.layers[&layer_id].transform_state.rect.left(), 0.0);
    }

    #[test]
    fn no_op_one_shot_edit_does_not_create_history() {
        let (mut state, _) = state_with_layer();
        let mut history = CanvasHistoryManager::with_initial_state(state.clone());

        history.apply(CanvasHistoryKind::DeleteLayers, &mut state, |_| {});

        assert_eq!(history.history_len(), 0);
    }

    #[test]
    fn one_shot_add_can_be_undone_and_redone() {
        let mut state = CanvasState::new();
        let mut history = CanvasHistoryManager::with_initial_state(state.clone());
        let layer = Layer::new_rectangle_shape_layer();
        let layer_id = layer.id;

        history.apply(CanvasHistoryKind::AddShape, &mut state, |state| {
            state.add_layer(layer, false);
        });

        assert_eq!(history.history_len(), 1);
        assert!(state.layers.contains_key(&layer_id));
        history.undo(&mut state);
        assert!(!state.layers.contains_key(&layer_id));
        history.redo(&mut state);
        assert!(state.layers.contains_key(&layer_id));
    }

    #[test]
    fn paste_is_one_undoable_edit_with_fresh_widget_ids() {
        let mut state = CanvasState::new();
        let original = Layer::new_rectangle_shape_layer();
        let original_id = original.id;
        let original_widget_id = original.transform_state.id;
        state.layers.insert(original_id, original.clone());
        let mut history = CanvasHistoryManager::with_initial_state(state.clone());

        history.apply(CanvasHistoryKind::Paste, &mut state, |state| {
            state.paste_layers(vec![original], Vec2::splat(20.0));
        });

        assert_eq!(history.history_len(), 1);
        assert_eq!(state.layers.len(), 2);
        let pasted = state
            .layers
            .values()
            .find(|layer| layer.id != original_id)
            .unwrap();
        assert!(pasted.selected);
        assert_ne!(pasted.transform_state.id, original_widget_id);

        history.undo(&mut state);
        assert_eq!(state.layers.len(), 1);
        assert!(state.layers.contains_key(&original_id));
    }

    #[test]
    fn crop_is_undoable_with_the_original_geometry() {
        let mut state = CanvasState::new();
        let mut layer = Layer::new_rectangle_shape_layer();
        let layer_id = layer.id;
        let original_crop = Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0));
        layer.content = LayerContent::Photo(CanvasPhoto {
            photo: test_photo(),
            adjustments: PhotoAdjustments::default(),
            crop: original_crop,
        });
        state.layers.insert(layer_id, layer);
        let original_rect = state.layers[&layer_id].transform_state.rect;
        let mut history = CanvasHistoryManager::with_initial_state(state.clone());
        let crop = Rect::from_min_max(egui::pos2(0.25, 0.25), egui::pos2(0.75, 0.75));

        history.apply(CanvasHistoryKind::Crop, &mut state, |state| {
            state.apply_crop(layer_id, crop);
        });
        let LayerContent::Photo(photo) = &state.layers[&layer_id].content else {
            unreachable!();
        };
        assert_eq!(photo.crop, crop);

        history.undo(&mut state);
        let LayerContent::Photo(photo) = &state.layers[&layer_id].content else {
            unreachable!();
        };
        assert_eq!(photo.crop, original_crop);
        assert_eq!(state.layers[&layer_id].transform_state.rect, original_rect);
    }

    #[test]
    fn undoing_delete_restores_quick_layout_order() {
        let mut state = CanvasState::new();
        let mut first = Layer::with_photo(test_photo());
        first.selected = true;
        let first_id = first.id;
        let second = Layer::with_photo(test_photo());
        let second_id = second.id;
        state.layers.insert(first_id, first);
        state.layers.insert(second_id, second);
        state.quick_layout_order = vec![first_id, second_id];
        let mut history = CanvasHistoryManager::with_initial_state(state.clone());

        history.apply(CanvasHistoryKind::DeleteLayers, &mut state, |state| {
            state.delete_selected_editable_layers();
        });
        assert_eq!(state.quick_layout_order, vec![second_id]);

        history.undo(&mut state);
        assert_eq!(state.quick_layout_order, vec![first_id, second_id]);
    }

    #[test]
    fn layer_order_is_historical() {
        let mut state = CanvasState::new();
        let first = Layer::new_rectangle_shape_layer();
        let first_id = first.id;
        let second = Layer::new_rectangle_shape_layer();
        let second_id = second.id;
        state.layers.insert(first_id, first);
        state.layers.insert(second_id, second);
        let mut history = CanvasHistoryManager::with_initial_state(state.clone());

        history.apply(CanvasHistoryKind::EditLayers, &mut state, |state| {
            state.layers.swap_indices(0, 1);
        });

        assert_eq!(
            state.layers.keys().copied().collect::<Vec<_>>(),
            vec![second_id, first_id]
        );
        assert_eq!(history.history_len(), 1);

        history.undo(&mut state);
        assert_eq!(
            state.layers.keys().copied().collect::<Vec<_>>(),
            vec![first_id, second_id]
        );
    }

    #[test]
    fn handle_mode_and_transient_transform_state_are_not_historical() {
        let (mut state, layer_id) = state_with_layer();
        let mut history = CanvasHistoryManager::with_initial_state(state.clone());

        history.apply(CanvasHistoryKind::Transform, &mut state, |state| {
            let transform = &mut state.layers.get_mut(&layer_id).unwrap().transform_state;
            transform.handle_mode = TransformHandleMode::Resize(ResizeMode::ConstrainedAspectRatio);
            transform.is_moving = true;
            transform.last_frame_rotation = 1.0;
            transform.change_in_rotation = Some(0.5);
        });

        assert_eq!(history.history_len(), 0);
    }

    #[test]
    fn consecutive_adjustment_drags_create_separate_entries() {
        struct HarnessState {
            canvas: CanvasState,
            history: CanvasHistoryManager,
            adjustments: PhotoAdjustmentsState,
        }

        fn drag_first_slider(harness: &mut Harness<'_, HarnessState>, delta_x: f32) {
            let rect = harness
                .get_all_by_role(egui::accesskit::Role::Slider)
                .next()
                .unwrap()
                .rect();
            let start = rect.center();
            let end = start + Vec2::X * delta_x;
            harness.hover_at(start);
            harness.run();
            harness.drag_at(start);
            harness.run();
            for step in 1..=4 {
                harness.hover_at(start.lerp(end, step as f32 / 4.0));
                harness.run();
            }
            harness.drop_at(end);
            harness.run();
        }

        let mut canvas = CanvasState::new();
        let mut layer = Layer::new_rectangle_shape_layer();
        layer.selected = true;
        layer.content = LayerContent::Photo(CanvasPhoto {
            photo: test_photo(),
            adjustments: PhotoAdjustments::default(),
            crop: Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0)),
        });
        canvas.layers.insert(layer.id, layer);
        let history = CanvasHistoryManager::with_initial_state(canvas.clone());
        let mut harness = Harness::new_ui_state(
            |ui, state: &mut HarnessState| {
                ui.set_width(320.0);
                let HarnessState {
                    canvas,
                    history,
                    adjustments,
                } = state;
                let photo = canvas.layers.values_mut().find_map(|layer| {
                    let LayerContent::Photo(photo) = &mut layer.content else {
                        return None;
                    };
                    Some(photo)
                });
                if let Some(photo) = photo {
                    let response =
                        PhotoAdjustmentsEditor::new(&mut photo.adjustments, adjustments).show(ui);
                    history.record(CanvasHistoryKind::AdjustPhoto, canvas, response);
                }
            },
            HarnessState {
                canvas,
                history,
                adjustments: PhotoAdjustmentsState::new(),
            },
        );

        harness
            .get_by_role_and_label(egui::accesskit::Role::Button, "Light")
            .click();
        harness.run();
        drag_first_slider(&mut harness, 30.0);
        assert_eq!(harness.state().history.history_len(), 1);
        drag_first_slider(&mut harness, -20.0);
        assert_eq!(harness.state().history.history_len(), 2);
        assert!(
            harness
                .state()
                .history
                .stack
                .history
                .iter()
                .all(|(kind, _)| *kind == CanvasHistoryKind::AdjustPhoto)
        );
    }

    #[test]
    fn changing_numeric_fields_creates_one_entry_per_field() {
        struct HarnessState {
            canvas: CanvasState,
            history: CanvasHistoryManager,
        }

        let mut canvas = CanvasState::new();
        let mut layer = Layer::new_rectangle_shape_layer();
        layer.selected = true;
        let layer_id = layer.id;
        canvas.layers.insert(layer_id, layer);
        let original_rect = canvas.layers[&layer_id].transform_state.rect;
        let history = CanvasHistoryManager::with_initial_state(canvas.clone());
        let mut harness = Harness::new_ui_state(
            |ui, state: &mut HarnessState| {
                let HarnessState { canvas, history } = state;
                let response = CanvasProperties {
                    canvas_state: canvas,
                }
                .show(ui);
                history.record(CanvasHistoryKind::EditProperties, canvas, response);
            },
            HarnessState { canvas, history },
        );

        harness
            .get_all_by_role(egui::accesskit::Role::TextInput)
            .next()
            .unwrap()
            .click();
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.run();
        harness
            .get_all_by_role(egui::accesskit::Role::TextInput)
            .next()
            .unwrap()
            .type_text("10");
        harness.run();
        harness.key_press(egui::Key::Tab);
        harness.run();
        assert_eq!(harness.state().history.history_len(), 1);

        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.run();
        harness
            .get_all_by_role(egui::accesskit::Role::TextInput)
            .nth(1)
            .unwrap()
            .type_text("20");
        harness.run();
        harness.key_press(egui::Key::Tab);
        harness.run();
        assert_eq!(harness.state().history.history_len(), 2);

        let state = harness.state_mut();
        state.history.undo(&mut state.canvas);
        assert_eq!(
            state.canvas.layers[&layer_id].transform_state.rect.top(),
            original_rect.top()
        );
        assert_eq!(
            state.canvas.layers[&layer_id].transform_state.rect.left(),
            10.0
        );
    }
}
