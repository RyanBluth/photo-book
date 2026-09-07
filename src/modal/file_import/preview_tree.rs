use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

/// A display row owns only its path, never a clone of an entire subtree.
#[derive(Debug)]
pub(super) struct PreviewTreeItem {
    pub(super) path: PathBuf,
    pub(super) depth: usize,
    pub(super) is_directory: bool,
    pub(super) has_children: bool,
}

#[derive(Default)]
pub(super) struct PreviewTree {
    children: BTreeMap<OsString, PreviewTree>,
    is_file: bool,
}

impl PreviewTree {
    pub(super) fn insert(&mut self, path: &Path) {
        let mut node = self;
        for component in path.components() {
            if let Component::Normal(name) = component {
                node = node.children.entry(name.to_owned()).or_default();
            }
        }
        node.is_file = true;
    }

    pub(super) fn into_items(self, root: PathBuf) -> Vec<PreviewTreeItem> {
        let mut items = Vec::new();
        self.flatten(root, 0, &mut items);
        items
    }

    fn flatten(self, path: PathBuf, depth: usize, items: &mut Vec<PreviewTreeItem>) {
        // A conflicting planned file/directory is still shown as a directory so its
        // descendants remain visible; the conflict is reported separately.
        let is_directory = depth == 0 || !self.is_file || !self.children.is_empty();
        items.push(PreviewTreeItem {
            path: path.clone(),
            depth,
            is_directory,
            has_children: !self.children.is_empty(),
        });
        let (directories, files): (Vec<_>, Vec<_>) = self
            .children
            .into_iter()
            .partition(|(_, child)| !child.is_file || !child.children.is_empty());
        for (name, child) in directories.into_iter().chain(files) {
            child.flatten(path.join(name), depth + 1, items);
        }
    }
}
