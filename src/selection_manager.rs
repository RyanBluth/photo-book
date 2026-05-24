use std::{collections::HashSet, path::PathBuf};

use egui::Ui;

use crate::{
    deferred_work_manager::{DeferredWorkHandle, DeferredWorkManager},
    dependencies::{Dependency, SingletonFor},
};

#[derive(Debug, Clone, Copy, Default)]
pub struct SelectionModifiers {
    pub ctrl: bool,
    pub shift: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SelectionSnapshot {
    pub selected_paths: HashSet<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct SelectionChange {
    pub added_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
struct DeferredSelectionChange {
    change: SelectionChange,
    handle: DeferredWorkHandle,
}

#[derive(Debug, Clone, Default)]
pub struct SelectionManager {
    selected_paths: HashSet<PathBuf>,
    selection_anchor: Option<PathBuf>,
    last_selection_change: Option<DeferredSelectionChange>,
}

impl SelectionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn selected_paths(&self) -> &HashSet<PathBuf> {
        &self.selected_paths
    }

    pub fn snapshot(&self) -> SelectionSnapshot {
        SelectionSnapshot {
            selected_paths: self.selected_paths.clone(),
        }
    }

    pub fn last_frame_selection(&self) -> Option<SelectionChange> {
        let last_selection_change = self.last_selection_change.as_ref()?;
        let should_perform =
            Dependency::<DeferredWorkManager>::get().with_lock(|deferred_work_manager| {
                deferred_work_manager.should_perform(last_selection_change.handle)
            });

        should_perform.then(|| last_selection_change.change.clone())
    }

    pub fn clear(&mut self) -> SelectionSnapshot {
        self.selected_paths.clear();
        self.selection_anchor = None;
        self.last_selection_change = None;
        self.snapshot()
    }

    pub fn clear_this_frame(&mut self, ui: &mut Ui) -> SelectionSnapshot {
        self.clear();
        self.record_change(ui, SelectionChange::default())
    }

    pub fn remove_path(&mut self, path: &PathBuf) -> SelectionSnapshot {
        self.selected_paths.remove(path);
        if self.selection_anchor.as_ref() == Some(path) {
            self.selection_anchor = None;
        }
        self.snapshot()
    }

    pub fn select_path(
        &mut self,
        ui: &mut Ui,
        ordered_paths: &[PathBuf],
        clicked_path: &PathBuf,
        modifiers: SelectionModifiers,
    ) -> SelectionSnapshot {
        if !ordered_paths.iter().any(|path| path == clicked_path) {
            self.clear();
            return self.record_change(ui, SelectionChange::default());
        }

        let previous_paths = self.selected_paths.clone();

        if modifiers.shift {
            self.select_range(ordered_paths, clicked_path, modifiers.ctrl);
        } else {
            self.select_single(clicked_path, modifiers.ctrl);
        }

        let snapshot = self.snapshot();
        let selection_change =
            selection_change_since(&previous_paths, ordered_paths, &snapshot.selected_paths);

        self.record_change(ui, selection_change);
        snapshot
    }

    fn record_change(&mut self, ui: &mut Ui, change: SelectionChange) -> SelectionSnapshot {
        let snapshot = self.snapshot();
        let handle = Dependency::<DeferredWorkManager>::get()
            .with_lock_mut(|deferred_work_manager| deferred_work_manager.after_repaint(ui));

        self.last_selection_change = Some(DeferredSelectionChange { change, handle });

        snapshot
    }

    fn select_single(&mut self, clicked_path: &PathBuf, ctrl: bool) {
        if ctrl {
            if !self.selected_paths.remove(clicked_path) {
                self.selected_paths.insert(clicked_path.clone());
            }
        } else {
            self.selected_paths.clear();
            self.selected_paths.insert(clicked_path.clone());
        }

        self.selection_anchor = Some(clicked_path.clone());
    }

    fn select_range(&mut self, ordered_paths: &[PathBuf], clicked_path: &PathBuf, ctrl: bool) {
        let anchor = self
            .selection_anchor
            .clone()
            .filter(|anchor| ordered_paths.iter().any(|path| path == anchor))
            .unwrap_or_else(|| clicked_path.clone());

        if !ctrl {
            self.selected_paths.clear();
        }

        if let Some(range_paths) = range_paths(ordered_paths, &anchor, clicked_path) {
            self.selected_paths.extend(range_paths);
            self.selection_anchor = Some(anchor);
        } else {
            self.selected_paths.insert(clicked_path.clone());
            self.selection_anchor = Some(clicked_path.clone());
        }
    }
}

fn range_paths(
    ordered_paths: &[PathBuf],
    anchor: &PathBuf,
    clicked_path: &PathBuf,
) -> Option<Vec<PathBuf>> {
    let anchor_index = ordered_paths.iter().position(|path| path == anchor)?;
    let clicked_index = ordered_paths.iter().position(|path| path == clicked_path)?;
    let (start, end) = if anchor_index <= clicked_index {
        (anchor_index, clicked_index)
    } else {
        (clicked_index, anchor_index)
    };

    Some(ordered_paths[start..=end].to_vec())
}

fn selection_change_since(
    previous_paths: &HashSet<PathBuf>,
    ordered_paths: &[PathBuf],
    selected_paths: &HashSet<PathBuf>,
) -> SelectionChange {
    let added_paths = ordered_paths
        .iter()
        .filter(|path| selected_paths.contains(*path) && !previous_paths.contains(*path))
        .cloned()
        .collect();

    SelectionChange { added_paths }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(name: &str) -> PathBuf {
        PathBuf::from(name)
    }

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(|name| path(name)).collect()
    }

    #[test]
    fn shift_selects_range_from_anchor() {
        let ordered_paths = paths(&["a.jpg", "b.jpg", "c.jpg", "d.jpg"]);
        let mut manager = SelectionManager {
            selected_paths: HashSet::from([path("b.jpg")]),
            selection_anchor: Some(path("b.jpg")),
            ..Default::default()
        };
        let previous_paths = manager.selected_paths.clone();

        manager.select_range(&ordered_paths, &path("d.jpg"), false);
        let snapshot = manager.snapshot();
        let selection_change =
            selection_change_since(&previous_paths, &ordered_paths, &snapshot.selected_paths);

        assert_eq!(manager.selection_anchor, Some(path("b.jpg")));
        assert_eq!(
            snapshot.selected_paths,
            HashSet::from([path("b.jpg"), path("c.jpg"), path("d.jpg")])
        );
        assert_eq!(selection_change.added_paths, paths(&["c.jpg", "d.jpg"]));
    }

    #[test]
    fn ctrl_shift_adds_range_to_existing_selection() {
        let ordered_paths = paths(&["a.jpg", "b.jpg", "c.jpg", "d.jpg"]);
        let mut manager = SelectionManager {
            selected_paths: HashSet::from([path("a.jpg")]),
            selection_anchor: Some(path("c.jpg")),
            ..Default::default()
        };
        let previous_paths = manager.selected_paths.clone();

        manager.select_range(&ordered_paths, &path("d.jpg"), true);
        let snapshot = manager.snapshot();
        let selection_change =
            selection_change_since(&previous_paths, &ordered_paths, &snapshot.selected_paths);

        assert_eq!(
            snapshot.selected_paths,
            HashSet::from([path("a.jpg"), path("c.jpg"), path("d.jpg")])
        );
        assert_eq!(selection_change.added_paths, paths(&["c.jpg", "d.jpg"]));
    }

    #[test]
    fn ctrl_click_removing_selection_has_no_added_paths() {
        let ordered_paths = paths(&["a.jpg", "b.jpg"]);
        let mut manager = SelectionManager {
            selected_paths: HashSet::from([path("a.jpg")]),
            selection_anchor: Some(path("a.jpg")),
            ..Default::default()
        };
        let previous_paths = manager.selected_paths.clone();

        manager.select_single(&path("a.jpg"), true);
        let snapshot = manager.snapshot();
        let selection_change =
            selection_change_since(&previous_paths, &ordered_paths, &snapshot.selected_paths);

        assert!(snapshot.selected_paths.is_empty());
        assert!(selection_change.added_paths.is_empty());
    }

    #[test]
    fn shift_with_stale_anchor_selects_clicked_path() {
        let ordered_paths = paths(&["a.jpg", "b.jpg"]);
        let mut manager = SelectionManager {
            selected_paths: HashSet::from([path("old.jpg")]),
            selection_anchor: Some(path("old.jpg")),
            ..Default::default()
        };

        manager.select_range(&ordered_paths, &path("b.jpg"), false);
        let snapshot = manager.snapshot();

        assert_eq!(manager.selection_anchor, Some(path("b.jpg")));
        assert_eq!(snapshot.selected_paths, HashSet::from([path("b.jpg")]));
    }

    #[test]
    fn clear_resets_selection_anchor() {
        let mut manager = SelectionManager {
            selected_paths: HashSet::from([path("a.jpg")]),
            selection_anchor: Some(path("a.jpg")),
            ..Default::default()
        };

        let snapshot = manager.clear();

        assert!(snapshot.selected_paths.is_empty());
        assert_eq!(manager.selection_anchor, None);
    }
}
